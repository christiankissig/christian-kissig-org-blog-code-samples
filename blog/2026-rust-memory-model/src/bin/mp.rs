// Native harness for exhibit 2, relaxed message passing.
//
//     cargo run --release --bin mp -- relaxed         100000000
//     cargo run --release --bin mp -- release-acquire 100000000
//
// Counts runs where the reader saw the flag but not the payload: (f, d) ==
// (1, 0). Allowed for Relaxed; x86 never shows it, Arm does. Forbidden for
// Release/Acquire, and no CPU should show it.
//
// The design follows litmus7: each run gets its own fresh `data` and `flag`,
// taken from arrays that are reset between rounds, and the writer and the
// reader meet at a sense-reversing spin barrier before every run. Reusing one
// pair of locations for every run, reset by a third thread, keeps them hot in
// the caches and all but hides the reordering on Graviton.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering, Ordering::*};
use std::sync::Arc;
use std::thread;

const ITERS: usize = 500_000; // runs per round

struct SpinBarrier {
    count: AtomicUsize,
    sense: AtomicBool,
}

impl SpinBarrier {
    fn wait(&self, local: &mut bool) {
        *local = !*local;
        if self.count.fetch_add(1, AcqRel) == 1 {
            self.count.store(0, Relaxed);
            self.sense.store(*local, Release);
        } else {
            while self.sense.load(Acquire) != *local {
                std::hint::spin_loop();
            }
        }
    }
}

fn zeros(n: usize) -> Arc<Vec<AtomicUsize>> {
    Arc::new((0..n).map(|_| AtomicUsize::new(0)).collect())
}

fn reset(v: &[AtomicUsize]) {
    for a in v {
        a.store(0, Relaxed);
    }
}

fn run(rounds: usize, store: Ordering, load: Ordering) -> u64 {
    let bar = Arc::new(SpinBarrier { count: AtomicUsize::new(0), sense: AtomicBool::new(false) });
    let (data, flag) = (zeros(ITERS), zeros(ITERS));
    let (fs, ds) = (zeros(ITERS), zeros(ITERS));
    let mut stale = 0u64;

    for _ in 0..rounds {
        reset(&data);
        reset(&flag);

        let (b, d, f) = (bar.clone(), data.clone(), flag.clone());
        let writer = thread::spawn(move || {
            let mut s = false;
            for i in 0..ITERS {
                b.wait(&mut s);
                d[i].store(1, Relaxed);
                f[i].store(1, store);
            }
        });

        // The reader runs on this thread.
        let mut s = false;
        for i in 0..ITERS {
            bar.wait(&mut s);
            let rf = flag[i].load(load);
            let rd = data[i].load(Relaxed);
            fs[i].store(rf, Relaxed);
            ds[i].store(rd, Relaxed);
        }
        writer.join().unwrap();

        stale += (0..ITERS)
            .filter(|&i| fs[i].load(Relaxed) == 1 && ds[i].load(Relaxed) == 0)
            .count() as u64;
    }
    stale
}

fn main() {
    let mut args = std::env::args().skip(1);
    let relaxed = args.next().as_deref() != Some("release-acquire");
    let n: usize = args.next().map_or(10_000_000, |s| s.parse().unwrap());
    let rounds = n.div_ceil(ITERS);
    let (store, load) = if relaxed { (Relaxed, Relaxed) } else { (Release, Acquire) };
    let stale = run(rounds, store, load);
    println!("{}: {} runs  (f, d) == (1, 0) in {stale}",
        if relaxed { "relaxed MP" } else { "release/acquire MP" }, rounds * ITERS);
}
