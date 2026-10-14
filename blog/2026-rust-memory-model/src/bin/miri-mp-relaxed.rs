// Relaxed message passing for Miri: exits with status 1 as soon as one run
// sees the flag but not the payload, so `-Zmiri-many-seeds` lists the seeds on
// which Miri's sampler produced the stale read.
//
//     MIRIFLAGS="-Zmiri-many-seeds=0..64 -Zmiri-many-seeds-keep-going" \
//         cargo +nightly miri run --bin miri-mp-relaxed
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::thread;

fn main() {
    for run in 0..64 {
        let data = AtomicUsize::new(0);
        let flag = AtomicUsize::new(0);
        let (f, d) = thread::scope(|s| {
            s.spawn(|| {
                data.store(42, Relaxed);
                flag.store(1, Relaxed);
            });
            let f = flag.load(Relaxed);
            let d = data.load(Relaxed);
            (f, d)
        });
        if f == 1 && d == 0 {
            eprintln!("stale read: flag seen, payload not, on run {run}");
            std::process::exit(1);
        }
    }
}
