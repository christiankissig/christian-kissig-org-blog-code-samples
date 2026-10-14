//! Exhibit 3: IRIW with SC fences.
//!
//!     RUSTFLAGS="--cfg loom" cargo test --release --test exhibit3_iriw
//!     cargo +nightly miri test --test exhibit3_iriw
//!
//! (r1, r2, r3, r4) == (1, 0, 1, 0) means the readers disagree about which
//! write came first. Original C11 allows it; RC11's repair, adopted by C++20
//! and so by Rust, forbids it. Both tests assert it never happens. Under a
//! Miri from before the late-2024 fixes (rust-lang/miri#2301) the Miri test
//! could fail, reporting an outcome C++20 forbids.

#[cfg(loom)]
use loom::{sync::atomic::{fence, AtomicUsize, Ordering}, sync::Arc, thread};
#[cfg(not(loom))]
use std::{sync::atomic::{fence, AtomicUsize, Ordering}, sync::Arc, thread};

use Ordering::{Relaxed, SeqCst};

fn iriw() -> (usize, usize, usize, usize) {
    let x = Arc::new(AtomicUsize::new(0));
    let y = Arc::new(AtomicUsize::new(0));
    let (xw, yw) = (x.clone(), y.clone());
    let (x2, y2) = (x.clone(), y.clone());

    let t0 = thread::spawn(move || xw.store(1, Relaxed));
    let t1 = thread::spawn(move || yw.store(1, Relaxed));
    let t2 = thread::spawn(move || {
        let r1 = x2.load(Relaxed);
        fence(SeqCst);
        let r2 = y2.load(Relaxed);
        (r1, r2)
    });
    // T3 runs on this thread.
    let r3 = y.load(Relaxed);
    fence(SeqCst);
    let r4 = x.load(Relaxed);

    t0.join().unwrap();
    t1.join().unwrap();
    let (r1, r2) = t2.join().unwrap();
    (r1, r2, r3, r4)
}

#[cfg(loom)]
#[test]
fn readers_never_disagree_under_loom() {
    loom::model(|| assert_ne!(iriw(), (1, 0, 1, 0)));
}

#[cfg(not(loom))]
#[test]
fn readers_never_disagree_under_miri() {
    for _ in 0..200 {
        assert_ne!(iriw(), (1, 0, 1, 0));
    }
}
