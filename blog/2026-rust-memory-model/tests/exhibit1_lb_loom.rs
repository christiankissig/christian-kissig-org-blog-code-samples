//! Exhibit 1: load buffering under loom.
//!
//!     RUSTFLAGS="--cfg loom" cargo test --release --test exhibit1_lb_loom
//!
//! The test PASSES. loom explores every interleaving it can generate and never
//! reaches (1, 1), because it does not explore load buffering. Rust's memory
//! model allows (1, 1); the passing test is loom's model speaking, not Rust's.

#![cfg(loom)]

use loom::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use loom::sync::Arc;
use loom::thread;

#[test]
fn lb_relaxed_passes_under_loom() {
    loom::model(|| {
        let x = Arc::new(AtomicUsize::new(0));
        let y = Arc::new(AtomicUsize::new(0));
        let (x0, y0) = (x.clone(), y.clone());

        let t0 = thread::spawn(move || {
            let r0 = x0.load(Relaxed);
            y0.store(1, Relaxed);
            r0
        });
        let r1 = y.load(Relaxed);
        x.store(1, Relaxed);
        let r0 = t0.join().unwrap();

        assert!(!(r0 == 1 && r1 == 1)); // "both can't read 1 — nothing wrote yet"
    });
}
