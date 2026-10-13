// 64 runs of the program as written, for Miri: `cargo +nightly miri run --bin miri-as-written`.
fn main() { for _ in 0..64 { lbuaf::as_written(); } }
