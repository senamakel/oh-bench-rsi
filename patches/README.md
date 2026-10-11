# Local draft patches for vendored repositories

The harness never pushes to, merges into, or opens pull requests against the
vendored repositories (ADR 0004). Changes made there during a campaign live
on local `rsi/*` branches; they are exported here with `git format-patch` so
they can be reviewed and, by a human, upstreamed. Submodule gitlinks in this
repository stay at their upstream pins.

| Directory | Repository | Base | What |
| --- | --- | --- | --- |
| `tinyrsi/` | tinyhumansai/tinyrsi | `212c52a` (pin) | rsi-viz **Swarm view**: `lib/swarm.ts` fold of the swarm trace kinds, `components/SwarmView.tsx` (seat cards, interaction graph, transcript, tool calls, budget gauges), wired into `TraceView`; vitest coverage |
| `tinyrsi-cli/` | tinyhumansai/tinyrsi | `f2dd371` (pin) | `tinyrsi` binary: `--env KEY=VALUE` on `run`/`score`; incumbent score cache keyed on `env`; stale ledger lock takeover. Regression tests first; see [run-03](../docs/results/run-03.md) |
| `tinymemory/` | tinyhumansai/tinymemory | `7973cac` (OpenHuman's pin) | **Candidate, not yet measured**: `recall::excerpt` — long recall hits keep a lead plus the query-matching lines instead of their first 600 characters; failing test first, then the fix |

Apply with `git -C <checkout> am patches/<dir>/*.patch`.
