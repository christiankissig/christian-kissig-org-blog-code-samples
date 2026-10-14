//! Exhibit 2: relaxed message passing.
//!
//! Under loom (exhaustive over loom's model):
//!     RUSTFLAGS="--cfg loom" cargo test --release --test exhibit2_mp
//! Under Miri (sampling; the relaxed test reports how often it saw the stale read):
//!     cargo +nightly miri test --test exhibit2_mp -- --nocapture
//!
//! With Relaxed, (f, d) == (1, 0) is allowed: the flag is seen, the payload is
//! stale. With Release/Acquire it is forbidden. On real hardware, see the
//! native harness in src/bin/mp.rs: x86 never shows (1, 0), Arm does.

#[cfg(loom)]
use loom::{sync::atomic::{AtomicUsize, Ordering}, sync::Arc, thread};
#[cfg(not(loom))]
use std::{sync::atomic::{AtomicUsize, Ordering}, sync::Arc, thread};

use Ordering::{Acquire, Relaxed, Release};

/// One run: T0 publishes `data` behind `flag`, T1 reads the flag, then the data.
fn mp(store: Ordering, load: Ordering) -> (usize, usize) {
    let data = Arc::new(AtomicUsize::new(0));
    let flag = Arc::new(AtomicUsize::new(0));
    let (data0, flag0) = (data.clone(), flag.clone());

    let t0 = thread::spawn(move || {
        data0.store(42, Relaxed);
        flag0.store(1, store);
    });
    let f = flag.load(load);
    let d = data.load(Relaxed);
    t0.join().unwrap();
    (f, if d == 42 { 1 } else { 0 })
}

#[cfg(loom)]
mod under_loom {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

    /// Whether any execution loom explores ends in `outcome`. The flag is a
    /// std atomic, outside loom's model, and private to this call.
    fn reachable(f: fn() -> (usize, usize), outcome: (usize, usize)) -> bool {
        let seen = std::sync::Arc::new(AtomicBool::new(false));
        let s = seen.clone();
        loom::model(move || if f() == outcome { s.store(true, SeqCst) });
        seen.load(SeqCst)
    }

    #[test]
    fn relaxed_stale_read_is_allowed() {
        assert!(reachable(|| mp(Relaxed, Relaxed), (1, 0)));
    }

    #[test]
    fn release_acquire_stale_read_is_forbidden() {
        assert!(!reachable(|| mp(Release, Acquire), (1, 0)));
    }
}

#[cfg(not(loom))]
mod under_miri_or_native {
    use super::*;

    const RUNS: usize = 2000;

    fn count(store: Ordering, load: Ordering) -> usize {
        (0..RUNS).filter(|_| mp(store, load) == (1, 0)).count()
    }

    #[test]
    fn relaxed_stale_read_is_sampled() {
        // Allowed, so any count is correct, including 0. Miri may or may not
        // hit it on a given seed: that is the point of exhibit 2.
        println!("relaxed MP: (1, 0) in {} of {RUNS} runs", count(Relaxed, Relaxed));
    }

    #[test]
    fn release_acquire_stale_read_never_happens() {
        assert_eq!(count(Release, Acquire), 0);
    }
}
