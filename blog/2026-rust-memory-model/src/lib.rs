use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering::Relaxed};
use std::thread;

pub struct Payload { pub field: usize }

/// The program as written: each thread loads, then stores.
pub fn as_written() -> (usize, usize) {
    let x = AtomicUsize::new(0);
    let y = AtomicUsize::new(0);
    let p = AtomicPtr::new(Box::into_raw(Box::new(Payload { field: 42 })));
    let (r0, r1) = thread::scope(|s| {
        let t0 = s.spawn(|| {
            let r0 = x.load(Relaxed);
            y.store(1, Relaxed);
            if r0 == 1 { unsafe { drop(Box::from_raw(p.load(Relaxed))) } }
            r0
        });
        let t1 = s.spawn(|| {
            let r1 = y.load(Relaxed);
            x.store(1, Relaxed);
            if r1 == 1 {
                let _v = unsafe { (*p.load(Relaxed)).field };
            }
            r1
        });
        (t0.join().unwrap(), t1.join().unwrap())
    });
    if r0 == 0 { unsafe { drop(Box::from_raw(p.load(Relaxed))) } }
    (r0, r1)
}

/// The same program after the compiler hoists each relaxed store above the
/// relaxed load before it: different locations, no dependency, so allowed.
pub fn reordered() -> (usize, usize) {
    let x = AtomicUsize::new(0);
    let y = AtomicUsize::new(0);
    let p = AtomicPtr::new(Box::into_raw(Box::new(Payload { field: 42 })));
    let (r0, r1) = thread::scope(|s| {
        let t0 = s.spawn(|| {
            y.store(1, Relaxed);
            let r0 = x.load(Relaxed);
            if r0 == 1 { unsafe { drop(Box::from_raw(p.load(Relaxed))) } }
            r0
        });
        let t1 = s.spawn(|| {
            x.store(1, Relaxed);
            let r1 = y.load(Relaxed);
            if r1 == 1 {
                let _v = unsafe { (*p.load(Relaxed)).field };
            }
            r1
        });
        (t0.join().unwrap(), t1.join().unwrap())
    });
    if r0 == 0 { unsafe { drop(Box::from_raw(p.load(Relaxed))) } }
    (r0, r1)
}

/// Safe Rust refuses to share a raw pointer between threads, before the
/// memory model is ever consulted. This is the post's compile error, checked
/// by `cargo test --doc`:
///
/// ```compile_fail,E0277
/// use std::thread;
///
/// let mut data: usize = 0;
/// let p: *mut usize = &mut data;
///
/// // error[E0277]: `*mut usize` cannot be sent between threads safely
/// let h = thread::spawn(move || unsafe { *p = 1 });
/// h.join().unwrap();
/// ```
pub mod raw_pointer_is_not_send {}
