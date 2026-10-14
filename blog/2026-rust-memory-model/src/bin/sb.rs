// Native harness for store buffering (SB), the test that shows Rust is
// strictly weaker than sequential consistency.
//
//     cargo run --release --bin sb -- relaxed  100000000
//     cargo run --release --bin sb -- sc-fence 100000000
//
// T0: x := 1; r0 := y    T1: y := 1; r1 := x
// Counts runs where both loads read 0: (r0, r1) == (0, 0). Allowed for
// Relaxed, and both x86 and Arm show it. Forbidden with a SeqCst fence between
// each store and load. Same design as mp.rs: fresh locations for every run.
use std::sync::atomic::{fence, AtomicBool, AtomicUsize, Ordering::*};
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

fn run(rounds: usize, sc_fence: bool) -> u64 {
    let bar = Arc::new(SpinBarrier { count: AtomicUsize::new(0), sense: AtomicBool::new(false) });
    let (xs, ys) = (zeros(ITERS), zeros(ITERS));
    let (r0s, r1s) = (zeros(ITERS), zeros(ITERS));
    let mut both_zero = 0u64;

    for _ in 0..rounds {
        reset(&xs);
        reset(&ys);

        let (b, x, y, r0) = (bar.clone(), xs.clone(), ys.clone(), r0s.clone());
        let t0 = thread::spawn(move || {
            let mut s = false;
            for i in 0..ITERS {
                b.wait(&mut s);
                x[i].store(1, Relaxed);
                if sc_fence { fence(SeqCst) }
                r0[i].store(y[i].load(Relaxed), Relaxed);
            }
        });

        // T1 runs on this thread.
        let mut s = false;
        for i in 0..ITERS {
            bar.wait(&mut s);
            ys[i].store(1, Relaxed);
            if sc_fence { fence(SeqCst) }
            r1s[i].store(xs[i].load(Relaxed), Relaxed);
        }
        t0.join().unwrap();

        both_zero += (0..ITERS)
            .filter(|&i| r0s[i].load(Relaxed) == 0 && r1s[i].load(Relaxed) == 0)
            .count() as u64;
    }
    both_zero
}

fn main() {
    let mut args = std::env::args().skip(1);
    let sc_fence = args.next().as_deref() == Some("sc-fence");
    let n: usize = args.next().map_or(10_000_000, |s| s.parse().unwrap());
    let rounds = n.div_ceil(ITERS);
    let both_zero = run(rounds, sc_fence);
    println!("{}: {} runs  (r0, r1) == (0, 0) in {both_zero}",
        if sc_fence { "SB with SC fences" } else { "relaxed SB" }, rounds * ITERS);
}
