# run-02 — baseline

OpenHuman TinyMemory `rsi/main` = pinned `7973cac`, configuration as in
[the loop spec](../specs/rsi-memory-loop.md). Charged **$2.54** of $5.

## Result

| Split | n | Accuracy |
| --- | ---: | ---: |
| dev | 75 | **0.320** |
| test | 150 | **0.160** |

By question type (dev / test):

| Type | dev | test |
| --- | ---: | ---: |
| procedure | 0.64 (14) | 0.39 (28) |
| static-environment | 0.50 (20) | 0.10 (40) |
| dynamic-environment | 0.18 (17) | 0.12 (34) |
| static-environment-abs | 0.10 (10) | 0.05 (21) |
| procedure-abs | 0.14 (7) | 0.08 (13) |
| dynamic-environment-abs | 0.00 (7) | 0.21 (14) |

Mean recall pack: ~1,100 tokens (the 1,200-token default budget).
The dev/test gap is large for a stratified split; with n = 75 and 150 the
intervals are wide, and promotion decisions use paired per-question
comparisons on test only.

## What the loop learnt

- **The bottleneck is recall, not reading.** In 46 of 51 dev failures
  (90 %), none of the answer's phrases appear anywhere in the pack the
  reader saw; in only 3 is the whole answer present. 23 failures are
  abstentions ("unknown") on a non-empty pack. (Gold answers were used for
  this analysis by Claude only; the swarm never sees them.)
- **Every failing pack contains head-truncated items** (`…`): long agent
  turns are cut to their first 600 characters, which keeps the task/URL
  preamble and drops page content. This is exactly what the run-01
  candidate (`recall::excerpt`) changes; it is not yet measured.
- **Procedures recall best** (thought/action text sits at the head of each
  item); **static-environment facts** (page content deep in each item) are
  far worse on test (0.10).

## Costs and the configuration that held

- Ingest of 1,737 events: $2.49 including one wasted re-enrichment (below).
  Enrichment coverage: 1,392 / 1,737 events enriched; the rest
  dead-lettered by the 2-calls-per-event cap (still embedded and
  searchable, without extracted facts).
- Reader (225) $0.042, judge (72 LLM-scored questions) $0.005.

## Bugs found and fixed

| Problem | Fix |
| --- | --- |
| CortexDB enriches in waves (first chunk = events written in the first ~35 s); readiness via `events_enriched` never completes because dead-lettered events count as neither enriched nor failed | ready when `jobs_queued_total ≥ stored` and nothing pending, three quiet polls (memd + `lme2_snapshot.py`; regression test) |
| memd's 30-min settle deadline < real ingest time; the eval's cleanup would delete a paid ingest on failure | `MEMD_SETTLE_SECS` (default 3 h); failed evals keep their data dir; `lme2_snapshot.py` salvages a live container into the cache |
| `CORTEX_ENRICHMENT_BACKFILL=1` in the standard env re-enriched a restored snapshot (1,168 jobs, ~18 min) | removed from the standard env; cache re-keyed (`14684dcc…`) |
