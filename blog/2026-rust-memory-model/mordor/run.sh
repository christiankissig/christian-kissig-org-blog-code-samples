#!/usr/bin/env bash
#
# Run the LB+UAF litmus tests through MoRDor under C++20 (Rust's contract),
# RC11 and sMRD, and print how many executions each has and whether any of
# them carries undefined behaviour (the use-after-free).
#
#     MORDOR=/path/to/mordor mordor/run.sh
#
# MORDOR defaults to `mordor` on the PATH. Build it from
# https://github.com/christiankissig/mordor at commit 15b90ed
# (`git checkout 15b90ed && dune build`, binary at
# _build/default/cli/main.exe). Other versions may count executions
# differently; the verdicts should not change.
set -euo pipefail
MORDOR="${MORDOR:-mordor}"
HERE="$(cd "$(dirname "$0")" && pwd)"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

printf '%-24s %-6s %-11s %s\n' test model executions "undefined behaviour"
for t in lb-uaf lb-uaf-acquire lb-uaf-release-acquire; do
  for m in c20 rc11 smrd; do
    # Each file asserts `allow (ub) []`; point the assertion at one model.
    sed "s/allow (ub) \[\]/allow (ub) [$m]/" "$HERE/$t.lit" > "$TMP/$t.lit"
    out="$("$MORDOR" run --single "$TMP/$t.lit" 2>/dev/null)"
    ex=$(sed -n 's/^Executions: //p' <<<"$out")
    ub=$(sed -n 's/^Undefined Behavior: //p' <<<"$out")
    printf '%-24s %-6s %-11s %s\n' "$t" "$m" "$ex" "$ub"
  done
done
