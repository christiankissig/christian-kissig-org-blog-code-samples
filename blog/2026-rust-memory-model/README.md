# LB+UAF: a use-after-free the tools do not see

Code for [Silence Is Not a Verdict](https://christian.kissig.org/blog/2026-rust-memory-model/).

The post opens with flag-guarded reclamation cut down to two threads:

```rust
// T0                                   // T1
let r0 = x.load(Relaxed);               let r1 = y.load(Relaxed);
y.store(1, Relaxed);                    x.store(1, Relaxed);
if r0 == 1 { free(p) }                  if r1 == 1 { read(*p) }
```

On `r0 == r1 == 1` one thread frees the payload the other reads. Rust's memory
model allows that outcome (load buffering), but loom, Miri and x86 never
produce it. Because each relaxed store goes to a different location from the
load before it and does not depend on it, a compiler may hoist the store above
the load:

```rust
// T0                                   // T1
y.store(1, Relaxed);                    x.store(1, Relaxed);
let r0 = x.load(Relaxed);               let r1 = y.load(Relaxed);
if r0 == 1 { free(p) }                  if r1 == 1 { read(*p) }
```

The reordered program is a valid compilation of the original, and an ordinary
interleaving of it reaches the use-after-free. This crate runs both versions.

## Layout

| File | What it does |
|---|---|
| `src/lib.rs` | `as_written()` and `reordered()`: one run of each program, threads spawned per run |
| `src/bin/miri-as-written.rs`, `src/bin/miri-reordered.rs` | 64 runs of each program, for Miri |
| `src/bin/harness.rs` | A litmus-style harness: two long-lived threads meet at a spin barrier for every run and count the outcomes |

## Requirements

- Rust stable for the native harness (tested with 1.99.0).
- Rust nightly with Miri and AddressSanitizer support (tested with nightly
  2026-10-11): `rustup toolchain install nightly --component miri rust-src`.
- An x86-64 Linux machine for the AddressSanitizer run. On other targets,
  replace `x86_64-unknown-linux-gnu` with your host triple.

## Running it

### Miri

```sh
# The program as written: no error on any seed.
MIRIFLAGS="-Zmiri-many-seeds=0..128" cargo +nightly miri run --bin miri-as-written

# The reordered program: undefined behaviour on some seeds.
MIRIFLAGS="-Zmiri-many-seeds=0..128 -Zmiri-many-seeds-keep-going" \
    cargo +nightly miri run --bin miri-reordered

# Seed 99 replays a use-after-free report.
MIRIFLAGS="-Zmiri-seed=99" cargo +nightly miri run --bin miri-reordered
```

On the reordered program, Miri reports either a use-after-free ("has been
freed, so this pointer is dangling") or a data race between the deallocation
and the read. Which one depends on whether the free or the read comes first in
the interleaving it picked. Both are the `r0 == r1 == 1` outcome.

### Native harness

```sh
cargo build --release
./target/release/harness written   10000000
./target/release/harness reordered 10000000
```

The harness prints how often each `(r0, r1)` outcome occurred. In the
reordered program, every `(1,1)` run frees memory the other thread then reads.
Without a sanitizer, that read usually returns stale data silently.

### AddressSanitizer

```sh
RUSTFLAGS="-Zsanitizer=address" \
    cargo +nightly build --release --target x86_64-unknown-linux-gnu
./target/x86_64-unknown-linux-gnu/release/harness written   10000000   # clean
./target/x86_64-unknown-linux-gnu/release/harness reordered 10000000   # heap-use-after-free
```

## Results recorded for the post

Intel Core i7-13700H, Linux, rustc 1.99.0 (native) and nightly 2026-10-11
(Miri, ASan):

| | As written | Reordered |
|---|---|---|
| Miri, 128 seeds × 64 runs | 0 seeds fail | 4 seeds fail (seeds 86, 97, 99, 124) |
| Native harness, `(1,1)` per 10M runs, three batches | 0, 0, 0 | 107, 32, 61 |
| AddressSanitizer, 10M runs | clean | heap-use-after-free within the first second |

Counts on other machines will differ. Reports from other platforms, Arm in
particular, are welcome at contact@kissig.org.

## Licence

Proprietary, all rights reserved; see [LICENSE](../../LICENSE).
