# run-03: first `tinyrsi` binary runs (10-question smoke)

Two `tinyrsi run loops/lme2-recall --epochs 1 --env LME2_LIMIT=5` runs: the
first five dev and five test questions of the frozen subset, one epoch, the
lead + `implementer-a` hive on GLM 5.3 Flash. A 10-question score is a smoke
measurement, not comparable with run-02's 225 questions.

| | Run | Charged | Outcome |
| --- | --- | ---: | --- |
| dry score | `score-dry-20261011-160933` | $0.88 | stopped by hand: the restored ingest re-enriched (see below) |
| run A | `20261011-162021` | $0.96 | baseline **dev 0.20 · test 0.20**; candidate rejected by the green gate |
| run B | `20261011-165250` | $0.04 | baseline from tinyrsi's cache; candidate rejected by the leakage gate |

Total in `~/.tinyrsi/spend.json`: **$1.87** (on top of run-00–02's $4.66:
$6.53 of the $50 campaign).

## What happened

- **The cached ingest is not settled.** `runs/ingest-cache/14684dcc…` was
  salvaged with a backfill, and its CortexDB keeps ~1,168 queued enrichment
  jobs; every restore drains them again (~$0.8 of GLM calls) before memd
  reports ready. `scorers/lme2.py` now re-caches a restored ingest once it has
  settled, so the queue is paid for once. Neither run scored a candidate, so
  the cache has not been refreshed yet: the next scored candidate pays the
  drain one last time.
- **Run A — the implementer ran out of turn.** The lead assigned a general
  fix (per-section line budgets and trimming the most expensive section
  first). The implementer used its 40 tool calls, ended mid-task, and its
  final text did not wake the lead (only `hivemind_send_agent` schedules a
  turn), so the hive went idle. tinyrsi auto-committed the half-done work;
  its new test read a private constant and `gates/green.py` rejected it.
  Fix: implementer `max_iterations` 40 → 120, and both seat prompts now say
  how to report and that unfinished work goes back.
- **Run B — the hive worked; the leakage gate fired.** The lead proposed
  dropping near-duplicate learnings from the pack (the baseline packs repeat
  one "disposition profile" bullet per question); the implementer committed it
  with three tests and green contract commands, and reported back. The diff's
  test fixture `"prefers dark mode in the editor"` contains a gold answer
  fragment, so `gates/leakage.py` rejected it. That is the gate doing its job
  conservatively; it was not loosened.

## Follow-ups

- The lead's action dir is the incumbent, so it approves on the
  implementer's report without reading the diff; give it read access to the
  candidate checkouts.
- A gate rejection reaches the next epoch's brief only as a count; feeding
  the reason back (e.g. "a test string matched benchmark content") would let
  the hive retry without the gate revealing what matched.

## tinyrsi changes (local `rsi/env-override`, exported to `patches/tinyrsi-cli/`)

- `--env KEY=VALUE` on `run` and `score`, merged into the frozen loop copy.
- The incumbent score cache is keyed on `env` (a 10-question score could
  otherwise stand in for a full one) — regression test first.
- A ledger lock left by a killed process (its pid no longer runs) is taken
  over instead of blocking every later run — regression test first.
