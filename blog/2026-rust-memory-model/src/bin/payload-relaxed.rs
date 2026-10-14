//! A non-atomic payload published behind a Relaxed flag: a data race.
//!
//!     cargo +nightly miri run --bin payload-relaxed    # data race, on every run
//!
//! The reader spins until it sees the flag. On x86 the payload write is then
//! always visible; on Arm it need not be. Either way the two accesses are not
//! ordered by happens-before, so this is undefined behaviour, and Miri's race
//! detector reports it on every run regardless of the schedule.
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::thread;

struct SendPtr(*mut usize);
unsafe impl Send for SendPtr {}

static FLAG: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let data: *mut usize = Box::into_raw(Box::new(0));
    let dp = SendPtr(data);
    let flag = &FLAG;

    let w = thread::spawn(move || {
        let dp = dp;
        unsafe { *dp.0 = 1 }        // non-atomic payload
        flag.store(1, Relaxed);     // Relaxed: no synchronizes-with edge
    });

    while flag.load(Relaxed) == 0 { // spin until the flag is observed
        std::hint::spin_loop();
    }
    let _rd = unsafe { *data };     // RACES with the writer's *dp.0 = 1

    w.join().unwrap();
    unsafe { drop(Box::from_raw(data)) }
}
