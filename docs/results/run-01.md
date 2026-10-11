# run-01 — Claude test run (setup, swarm smoke, first ingests)

Charged **$1.11** of $5 (settled $1.08; $0.03 uncertain). Spend by phase:
ingest $1.063 (3,201 calls), swarm $0.021 (32 calls).

## What was proven

- **Swarm wiring.** The `memory-rsi` hive (lead + implementer-a, GLM 5.3
  Flash) ran end to end: the lead read the brief, delegated over a direct
  message, the implementer ran `cargo test -p tinymemory-tools` in its own
  worktree (156 passed) and reported back, and the lead closed the epoch.
  6 turns, 0 failures, $0.021.
- **Gate.** The hand-made candidate passes; planted candidates that edit the
  frozen api, embed a test question, or change no test are all rejected.
- **Visualizer.** `apps/rsi-viz` renders the swarm trace live (seats,
  interaction graph, transcript, tool calls, budget gauges).

## What broke, and the fixes

| Problem | Fix |
| --- | --- |
| Coordinator default `round_width = 1`: the lead's re-nudged episode starved the implementer | use the package's coordinator settings (`round_width` 4) |
| Seats waited/polled inside their turn | deny `wait`/`wait_loop`; prompt: end the turn after sending; kickoff as a direct message so nothing re-nudges |
| Trace missed seat-to-seat DMs | mirror the host-scope transcript |
| memd declared readiness between CortexDB's 256-event enrichment waves | wait for `events_enriched ≥ stored` plus three quiet polls (regression tests) |
| CortexDB state lives under `/data`; only `/data/cortex` was mounted, so a restart lost the index | persist the whole data root; restart/recall verified |
| Full enrichment costs ~$0.004/event (~$7 for the haystack) | entity graph off, enrichment input ≤ 3 KB/event, ≤ 2 LLM calls/event |

The two partial ingests (~$1.06) were lost to the mount and wave issues.

## Hand-made candidate (`rsi/cand-run-01-claude`, TinyMemory)

`recall::excerpt`: long hits keep a short lead plus the lines sharing the
most query terms, instead of being cut to their first 600 characters. Test
first (`a_long_hit_keeps_the_line_that_answers_the_query` failed on
`rsi/main`), then the fix; TinyMemory suite 1,298 passed. Evaluated in a
later run against the baseline.
