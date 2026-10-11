# RSI memory loop

- **Status:** Accepted (implemented as a tinyrsi loop folder; ADR 0007)
- **Owner:** oh-bench-rsi maintainers
- **Loop:** [`loops/lme2-recall/`](../../loops/lme2-recall/README.md)
- **Runner contract:** [tinyrsi `rsi-cli` spec](../../vendor/tinyrsi/docs/specs/rsi-cli.md)

## Problem

OpenHuman's memory had never been measured on LongMemEval-V2, and its known
recall weaknesses (evidence lost in pack trimming, long items crowding out
others, temporal ordering) are many small, interacting design choices — a
search space suited to an automated, disciplined improvement loop, provided
the loop cannot win by changing the measurement.

## Goals and non-goals

- Goal: raise held-out LongMemEval-V2 accuracy by changing only TinyMemory's
  recall-side crates, with every change tested and reviewable.
- Goal: this repository owns only the benchmark; the loop machinery is
  tinyrsi's.
- Non-goal: comparability with the LongMemEval-V2 leaderboard (see the
  measurement configuration below).
- Non-goal: automatic upstreaming. Promotions stay local (ADR 0004).

## Behaviour

`tinyrsi run loops/lme2-recall` runs at most `epochs` (2) epochs, stopping
early after `patience` (2) epochs without a promotion or when the budget is
spent. Every model call goes through the run's spend gateway, capped at
`budget.run_usd` ($5) per run and `budget.total_usd` ($50) across runs.

1. **Measure the incumbent** — TinyMemory's local `rsi/main` in
   `vendor/openhuman/vendor/tinymemory` — with `scorers/lme2.py`. tinyrsi
   caches the result per commit, loop and scorer definition.
2. **Brief** — `brief.md` filled from the incumbent's `result.json`: dev
   metrics, per-type dev accuracy and a sample of failed dev questions (the
   question, the model's answer, the recall pack). No gold answers, no
   per-question test detail.
3. **Swarm** — the `hive/` package (tinyhivemind hive language) runs as
   OpenHuman agents on GLM 5.3 Flash. The lead reads the incumbent; the
   implementer edits its own TinyMemory worktree on
   `rsi/cand-<run>-e<n>-<seat>`.
4. **Check, before any score spend**, in order:
   - surface: changed paths only under `crates/tinymemory-tools/` or
     `crates/tinymemory-integrations/`, none under `crates/tinymemory-api/`,
     and at least one test path changed (tinyrsi);
   - leakage (`gates/leakage.py`): no added line contains a subset question
     window (40 chars), a gold answer fragment (≥ 6 chars) or a benchmark
     canary;
   - green (`gates/green.py`): `cargo test -p <crate>` passes for every
     touched crate.
5. **Measure the candidate** on the same 225 questions, if
   `budget.score_reserve_usd` ($0.60) is still affordable.
6. **Decide** on the 150 test questions only: paired per-question 0/1
   scores, exact one-sided sign test on discordant pairs at α = 0.10.
7. **Promote** the accepted candidate with the best `lme2` score by
   fast-forwarding the local `rsi/main`. Nothing is pushed, merged upstream
   or opened as a PR.

Between runs a human (or Claude) reviews the run store, improves the hive,
the scorer plumbing and the loop folder, and records each run in
`docs/results/`. A promoted change is upstreamed by hand from `patches/`.

## Scorer output

`scorers/lme2.py` writes `$RSI_OUT/result.json`:

| Field | Value |
| --- | --- |
| `score` | dev accuracy |
| `splits.dev.items`, `splits.test.items` | per-question 0/1 scores |
| `failures` | up to 30 dev failures, ≤ 4 per question type: `{id, type, text}` |
| `metrics` | `dev_accuracy`, `test_accuracy`, `dev_pack_tokens_mean`, `dev_accuracy_<type>` |

A failure's `text` holds the question (≤ 600 chars), the model's answer with
an `[answered unknown]` marker, and the recall pack (≤ 1,500 chars) in a code
fence. tinyrsi shows at most ten, two per type, in the brief.

## Measurement configuration (non-official)

| Item | Value |
| --- | --- |
| Dataset | LongMemEval-V2 `f152293e…`, harness `2cc8c540…`, unmodified |
| Subset | web, small tier, text-only questions; 75 dev / 150 test, seed 27 (`campaigns/lme2/subset.json`) |
| Memory | OpenHuman TinyMemory `AgentMemory::recall`, OpenHuman's default recall policy (1,200-token pack) |
| Engine | CortexDB v0.10.4, local Docker, enrichment on, 16-way, one scan wave, no backfill |
| Ingest | one assistant turn per trajectory state: task, URL, thought, action, novel page lines (≤ 6,000 chars) |
| Reader / judge | `z-ai/glm-5.3-flash`, reasoning low, via the gateway |

This is not the paper's configuration (Qwen3.5-9B reader, GPT-5.2 judge,
images) and its scores are not comparable to the leaderboard.

## Invariants

- Only the gateway holds the provider key; every model call is metered and
  counted against both caps.
- `benchmarks/`, the official harness and the scorer's measurement (models,
  image, ingest environment, subset) are never edited to raise a score.
- An ingest snapshot is reused only for an identical haystack, engine image,
  ingest configuration and write path (TinyMemory api/integrations sources,
  the adapter's converter, the CortexDB config).
- The swarm never sees gold answers or per-question test results; gate
  reasons never quote matched benchmark text.

## Acceptance criteria

- `tinyrsi check loops/lme2-recall` prints `ok: lme2-recall ...`.
- The loop scripts' unit tests pass offline.
- Baseline and at least one candidate measured end to end within the caps.
- Every promotion carries a positive sign-test decision on the test split.
- Runs are visible live in the `tinyrsi` UI.
