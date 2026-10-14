# Code samples for christian.kissig.org

Working code behind posts on [christian.kissig.org](https://christian.kissig.org),
with the instructions to build and run it. Each directory mirrors the post's
path in the site's `content/` directory, so the code for
`https://christian.kissig.org/blog/2026-rust-memory-model/` lives in
[`blog/2026-rust-memory-model`](blog/2026-rust-memory-model).

| Directory | Post | What is in it |
|---|---|---|
| [`blog/2026-rust-memory-model`](blog/2026-rust-memory-model) | [Silence Is Not a Verdict](https://christian.kissig.org/blog/2026-rust-memory-model/) | Every code block in the post: LB+UAF as written and reordered, the three exhibits (load buffering, relaxed message passing, IRIW with SC fences), and the safe/unsafe examples, under loom, Miri, AddressSanitizer and native harnesses on x86 and Graviton |

Each directory has its own README with the toolchain it needs, the commands to
run, and the results recorded for the post.

Corrections and reports of results on other machines are welcome at
contact@kissig.org.

## Licence

Proprietary, all rights reserved; see [LICENSE](LICENSE). You may build and
run the code to reproduce the posts' results.
