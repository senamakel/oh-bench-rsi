# RSI memory loop

- **Status:** Accepted (implemented; live runs in progress)
- **Owner:** oh-bench-rsi maintainers

## Problem

OpenHuman's memory has never been measured on LongMemEval-V2, and its known
recall weaknesses (evidence lost in pack trimming, long items crowding out
others, temporal ordering) are many small, interacting design choices — a
search space suited to an automated, disciplined improvement loop, provided
the loop cannot win by changing the measurement.

## Behaviour

One **run** is capped at $5 through the spend gateway; the campaign at $50.
Each run executes `scripts/rsi/epoch.py`:

1. **Measure the incumbent** (local `rsi/main` of the vendored TinyMemory)
   once per commit; the summary is reused while the commit is unchanged.
2. **Brief**: dev-split accuracy by question type and a sample of failed dev
   questions with the model's answer and the recall pack it saw. No gold
   answers, no test-split detail.
3. **Swarm** (`swarm/`): the `hives/memory-rsi` package, written in the
   tinyhivemind hive language, runs as OpenHuman agents on GLM 5.3 Flash on
   a tinyhivemind coordinator. Editing seats work in their own TinyMemory
   worktree on `rsi/cand-<run>-e<n>-<seat>`, compile and test continuously,
   and commit.
4. **Gate** (`scripts/rsi/gate.py`), before any evaluation spend:
   surface (only `tinymemory-tools`/`tinymemory-integrations`), leakage
   (no subset question windows, gold answer phrases or site canaries in
   added lines), tests changed, and `cargo test` green.
5. **Measure the candidate** on the same 225 questions
   (`scripts/bench/lme2_eval.py`).
6. **Decide** (`scripts/rsi/decide.py`) on the 150 test questions only:
   paired per-question scores, exact one-sided sign test at α = 0.10.
7. **Promote** an accepted candidate by fast-forwarding the *local*
   `rsi/main`. Nothing is pushed, merged upstream or opened as a PR.

Between runs Claude reviews the traces, ledgers and gate results and
improves the RSI machinery, the hive package, OpenHuman/TinyMemory, and the
bench wiring (`docs/results/` records each run).

## Measurement configuration (non-official)

| Item | Value |
| --- | --- |
| Dataset | LongMemEval-V2 `f152293e…`, harness `2cc8c540…`, unmodified |
| Subset | web, small tier, text-only questions; 75 dev / 150 test, seed 27 |
| Memory | OpenHuman TinyMemory `AgentMemory::recall`, OpenHuman's default recall policy (1,200-token pack) |
| Engine | CortexDB v0.10.4, local Docker, enrichment on, 16-way, one scan wave |
| Ingest | one assistant turn per trajectory state: task, URL, thought, action, novel page lines (≤ 6,000 chars) |
| Reader / judge | `z-ai/glm-5.3-flash`, reasoning low, via the gateway |

This is not the paper's configuration (Qwen3.5-9B reader, GPT-5.2 judge,
images) and its scores are not comparable to the leaderboard.

## Invariants

- Only the gateway holds the provider key; every model call is metered and
  counted against both caps; the caps cannot be raised for an existing ledger.
- `benchmarks/` and the official harness are never edited.
- An ingest snapshot is reused only for an identical haystack, engine image,
  ingest configuration and write path (TinyMemory api/integrations sources,
  the adapter's converter).
- The swarm never sees gold answers or the test split.

## Acceptance criteria

- Baseline and at least one candidate measured end to end within the caps.
- Every promotion carries a positive paired decision on the test split.
- Swarm runs are visible in `apps/rsi-viz` (Swarm view) live.
