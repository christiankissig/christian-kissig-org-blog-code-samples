// Litmus-style harness: two long-lived threads meet at a spin barrier for each
// iteration, so the racing accesses start within a few cycles of each other.
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::thread;

struct Payload { field: usize }

struct Shared {
    x: AtomicUsize,
    y: AtomicUsize,
    p: AtomicPtr<Payload>,
    go: AtomicUsize,   // iteration the threads may start
    done: AtomicUsize, // threads finished with the current iteration
    r: [AtomicUsize; 2],
}

fn main() {
    let mut args = std::env::args().skip(1);
    let reordered = args.next().as_deref() == Some("reordered");
    let n: usize = args.next().map_or(1_000_000, |s| s.parse().unwrap());
    let s = Arc::new(Shared {
        x: AtomicUsize::new(0), y: AtomicUsize::new(0),
        p: AtomicPtr::new(std::ptr::null_mut()),
        go: AtomicUsize::new(0), done: AtomicUsize::new(0),
        r: [AtomicUsize::new(0), AtomicUsize::new(0)],
    });
    let spawn = |me: usize| {
        let s = s.clone();
        thread::spawn(move || {
            let (mine, theirs) = if me == 0 { (&s.y, &s.x) } else { (&s.x, &s.y) };
            for i in 1..=n {
                while s.go.load(Acquire) != i { std::hint::spin_loop() }
                let r = if reordered {
                    mine.store(1, Relaxed);
                    theirs.load(Relaxed)
                } else {
                    let r = theirs.load(Relaxed);
                    mine.store(1, Relaxed);
                    r
                };
                if r == 1 {
                    let p = s.p.load(Relaxed);
                    if me == 0 {
                        unsafe { drop(Box::from_raw(p)) }
                    } else {
                        let _v = unsafe { std::ptr::read_volatile(&(*p).field) };
                    }
                }
                s.r[me].store(r, Relaxed);
                s.done.fetch_add(1, AcqRel);
            }
        })
    };
    let (t0, t1) = (spawn(0), spawn(1));
    let mut c = [0usize; 4];
    for i in 1..=n {
        s.x.store(0, Relaxed); s.y.store(0, Relaxed); s.done.store(0, Relaxed);
        s.p.store(Box::into_raw(Box::new(Payload { field: 42 })), Relaxed);
        s.go.store(i, Release);
        while s.done.load(Acquire) != 2 { std::hint::spin_loop() }
        let (r0, r1) = (s.r[0].load(Relaxed), s.r[1].load(Relaxed));
        if r0 == 0 { unsafe { drop(Box::from_raw(s.p.load(Relaxed))) } }
        c[r0 * 2 + r1] += 1;
    }
    t0.join().unwrap(); t1.join().unwrap();
    println!("{}: {} runs  (0,0)={} (0,1)={} (1,0)={} (1,1)={}",
        if reordered { "reordered" } else { "as written" }, n, c[0], c[1], c[2], c[3]);
}
