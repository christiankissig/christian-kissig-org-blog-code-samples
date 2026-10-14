# Silence Is Not a Verdict: code samples

Runnable versions of every code block in
[Silence Is Not a Verdict](https://christian.kissig.org/blog/2026-rust-memory-model/).
The post shows each program as a sketch, one column per thread. This crate has
the full programs, and the commands that produce the results quoted in the
post.

| In the post | Here | Run with |
|---|---|---|
| The opening example, LB+UAF, as written and reordered | `src/lib.rs`, `src/bin/miri-*.rs`, `src/bin/harness.rs` | Miri, native, AddressSanitizer |
| Exhibit 1: load buffering under loom | `tests/exhibit1_lb_loom.rs` | loom |
| Exhibit 2: relaxed message passing | `tests/exhibit2_mp.rs`, `src/bin/miri-mp-relaxed.rs`, `src/bin/mp.rs` | loom, Miri, native |
| Exhibit 3: IRIW with SC fences | `tests/exhibit3_iriw.rs` | loom, Miri |
| `*mut usize` is not `Send` (E0277) | doctest on `raw_pointer_is_not_send` in `src/lib.rs` | `cargo test --doc` |
| A non-atomic payload behind a flag | `src/bin/payload-relaxed.rs`, `src/bin/payload-release-acquire.rs` | Miri |

## Requirements

- Rust stable for the native harnesses and loom (tested with 1.99.0).
- Rust nightly with Miri and AddressSanitizer support (tested with nightly
  2026-10-11): `rustup toolchain install nightly --component miri rust-src`.
- For AddressSanitizer, an x86-64 or AArch64 Linux machine. The commands below
  use `x86_64-unknown-linux-gnu`; on Arm use `aarch64-unknown-linux-gnu`.
- For the Graviton runs, a configured `aws` CLI plus `ssh`, `scp`, `curl` and
  `jq` (see [Running on Graviton](#running-on-graviton)).

loom is only compiled in under `--cfg loom`, so a plain build has no
dependencies.

## The opening example: LB+UAF

Flag-guarded reclamation, cut down to two threads:

```rust
// T0                                   // T1
let r0 = x.load(Relaxed);               let r1 = y.load(Relaxed);
y.store(1, Relaxed);                    x.store(1, Relaxed);
if r0 == 1 { free(p) }                  if r1 == 1 { read(*p) }
```

On `r0 == r1 == 1` one thread frees the payload the other reads. Rust's memory
model allows that outcome (load buffering). Each relaxed store goes to a
different location from the load before it and does not depend on it, so the
model allows a compiler, or an Arm core, to move the store above the load:

```rust
// T0                                   // T1
y.store(1, Relaxed);                    x.store(1, Relaxed);
let r0 = x.load(Relaxed);               let r1 = y.load(Relaxed);
if r0 == 1 { free(p) }                  if r1 == 1 { read(*p) }
```

Today, neither rustc 1.99 nor Graviton3 makes the move. rustc keeps the source
order in optimised LLVM IR and in x86-64 and AArch64 assembly. But an ordinary
interleaving of the reordered program reaches the use-after-free, and this
crate runs both versions. `as_written()` and `reordered()` in `src/lib.rs` are
one run of each.

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

Each binary runs its program 64 times, so 128 seeds make 8,192 runs. On the
reordered program, Miri reports either a use-after-free ("has been freed, so
this pointer is dangling") or a data race between the deallocation and the
read, depending on which comes first in the interleaving it picked. Both are
the `r0 == r1 == 1` outcome.

Miri interprets the source, not the compiled binary. It sees the bug in the
reordered program because the reordering is written into the source, and it
cannot see it in the program as written because it does not emulate load
buffering.

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

## Exhibit 1: load buffering under loom

```sh
RUSTFLAGS="--cfg loom" cargo test --release --test exhibit1_lb_loom
```

The test asserts that `(1, 1)` never happens, and it **passes**. loom explores
every interleaving it can generate, but it does not explore load buffering, so
it never reaches an outcome Rust's memory model allows. A passing loom test
here is loom's model speaking, not Rust's.

## Exhibit 2: relaxed message passing

```rust
// T0                              // T1
data.store(42, Relaxed);           let f = flag.load(Relaxed);
flag.store(1, Relaxed);            let d = data.load(Relaxed);
// (f, d) == (1, 0): the flag is seen, the payload is stale
```

```sh
# loom: the stale read is allowed with Relaxed, forbidden with Release/Acquire.
RUSTFLAGS="--cfg loom" cargo test --release --test exhibit2_mp

# Miri: samples. The Relaxed test prints how often it saw the stale read.
cargo +nightly miri test --test exhibit2_mp -- --nocapture

# Miri across seeds: the binary exits 1 when it sees the stale read.
MIRIFLAGS="-Zmiri-many-seeds=0..64 -Zmiri-many-seeds-keep-going" \
    cargo +nightly miri run --bin miri-mp-relaxed

# Native: x86 never shows it; Arm does.
cargo run --release --bin mp -- relaxed         100000000
cargo run --release --bin mp -- release-acquire 100000000
```

## Exhibit 3: IRIW with SC fences

```sh
RUSTFLAGS="--cfg loom" cargo test --release --test exhibit3_iriw
cargo +nightly miri test --test exhibit3_iriw
```

Both tests assert that the two readers never disagree about which write came
first, `(r1, r2, r3, r4) == (1, 0, 1, 0)`. Original C11 allows that outcome;
RC11's repair, adopted by C++20 and so by Rust, forbids it. Under a Miri from
before the late-2024 fixes
([rust-lang/miri#2301](https://github.com/rust-lang/miri/issues/2301)), the
Miri test could fail, reporting an outcome C++20 forbids.

## Safe Rust and `unsafe`

```sh
cargo test --doc   # the E0277 example fails to compile, as it should

cargo +nightly miri run --bin payload-relaxed           # data race, on every run
cargo +nightly miri run --bin payload-release-acquire   # clean
```

`payload-relaxed` publishes a non-atomic payload behind a `Relaxed` flag. The
reader spins until it sees the flag, but the two payload accesses are not
ordered by happens-before, so this is undefined behaviour, and Miri's race
detector reports it on every run, whatever the schedule. With
`Release`/`Acquire` the same program runs clean.

## Running on Graviton

```sh
REGION=us-east-2 aarch64/run-graviton.sh                             # c7g.2xlarge, Graviton3
INSTANCE_TYPE=c8g.2xlarge REGION=us-east-2 aarch64/run-graviton.sh   # Graviton4
```

The script is self-contained and ephemeral. It creates a throwaway key pair and
a security group locked to your public IP, launches one instance with a
60-minute self-terminate, builds the crate there, runs the LB+UAF and exhibit 2
harnesses natively and under AddressSanitizer, and writes the output to
`aarch64/results/`. An exit trap deletes the instance, the security group and
the key pair, even on error. It costs a few cents of EC2 time.

## Results recorded for the post

x86: Intel Core i7-13700H, Linux. Graviton3: AWS c7g.2xlarge, Neoverse-V1,
Amazon Linux 2023. rustc 1.99.0 for native runs and loom; nightly 2026-10-11
for Miri and AddressSanitizer.

| | Result |
|---|---|
| LB+UAF as written, Miri, 128 seeds × 64 runs | 0 seeds fail |
| LB+UAF reordered, Miri, 128 seeds × 64 runs | 4 seeds fail (86, 97, 99, 124) |
| LB+UAF as written, `(1,1)`, x86 | 0 in 30M (three batches of 10M) |
| LB+UAF reordered, `(1,1)`, x86 | 200 in 30M (107, 32, 61) |
| LB+UAF as written, `(1,1)`, Graviton3 | 0 in 200M, in each of two runs |
| LB+UAF reordered, `(1,1)`, Graviton3 | 7 in 30M (6, 1, 0); in a second run, 11 in 30M (3, 5, 3) |
| LB+UAF reordered, AddressSanitizer, x86 | heap-use-after-free within the first second |
| LB+UAF reordered, AddressSanitizer, Graviton3 | no report in two runs of 10M. One reached `(1,1)` twice; ASan reports only when the read comes after the free in real time |
| Exhibit 1, loom | test passes: `(1,1)` never explored |
| Exhibit 2, loom | Relaxed: stale read allowed. Release/Acquire: forbidden |
| Exhibit 2, Miri, `exhibit2_mp`, default seed | Relaxed: 0 stale reads in 2,000 runs. Release/Acquire: 0 |
| Exhibit 2, Miri, `miri-mp-relaxed`, 64 seeds × 64 runs | 1 seed sees the stale read (seed 8) |
| Exhibit 2, relaxed, x86 | 0 in 10M |
| Exhibit 2, relaxed, Graviton3 | 4,761,020 in 100M |
| Exhibit 2, release/acquire, x86 and Graviton3 | 0 |
| Exhibit 3, loom and Miri | readers never disagree |
| `payload-relaxed`, Miri | data race reported |
| `payload-release-acquire`, Miri | clean |

Raw Graviton output is in `aarch64/results/`:

- `graviton-20261014-073301.txt`: the LB+UAF runs quoted in the post.
- `graviton-20261014-095928.txt`: a second LB+UAF run, and a first version of
  the exhibit 2 harness that reused one pair of locations for every run. It
  saw 0 stale reads in 100M: keeping the locations hot in the caches hid the
  reordering. The harness design matters as much as the run count.
- `graviton-20261014-100521.txt`: the exhibit 2 harness as it is now, with
  fresh locations for every run, and short LB+UAF batches. Counts on other machines will
differ. Reports from other platforms, Arm in particular, are welcome at
contact@kissig.org.

## Licence

Proprietary, all rights reserved; see [LICENSE](../../LICENSE).
