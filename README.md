# Code samples for christian.kissig.org

Working code behind posts on [christian.kissig.org](https://christian.kissig.org),
with the instructions to build and run it. Each directory mirrors the post's
path in the site's `content/` directory, so the code for
`https://christian.kissig.org/blog/2026-rust-memory-model/` lives in
[`blog/2026-rust-memory-model`](blog/2026-rust-memory-model).

| Directory | Post | What is in it |
|---|---|---|
| [`blog/2026-rust-memory-model`](blog/2026-rust-memory-model) | [Silence Is Not a Verdict](https://christian.kissig.org/blog/2026-rust-memory-model/) | LB+UAF: flag-guarded reclamation with relaxed atomics, as written and after a legal compiler reordering, under Miri, AddressSanitizer and a native litmus harness |

Each directory has its own README with the toolchain it needs, the commands to
run, and the results recorded for the post.

Corrections and reports of results on other machines are welcome at
contact@kissig.org.

## Licence

Proprietary, all rights reserved; see [LICENSE](LICENSE). You may build and
run the code to reproduce the posts' results.
