//! The same payload published behind a Release/Acquire flag: no race.
//!
//!     cargo +nightly miri run --bin payload-release-acquire    # clean
//!
//! The Acquire load that sees the flag synchronizes with the Release store,
//! so the payload write happens-before the read on every machine.
use std::sync::atomic::{AtomicUsize, Ordering::{Acquire, Release}};
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
        flag.store(1, Release);     // Release: pairs with the Acquire below
    });

    while flag.load(Acquire) == 0 { // spin until the flag is observed
        std::hint::spin_loop();
    }
    let _rd = unsafe { *data };     // ordered after the write: no race

    w.join().unwrap();
    unsafe { drop(Box::from_raw(data)) }
}
