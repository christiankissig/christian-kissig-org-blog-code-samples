// 64 runs of the reordered program, for Miri: `cargo +nightly miri run --bin miri-reordered`.
fn main() { for _ in 0..64 { lbuaf::reordered(); } }
