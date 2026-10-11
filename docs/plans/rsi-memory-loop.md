# Plan: RSI memory loop

Spec: [`../specs/rsi-memory-loop.md`](../specs/rsi-memory-loop.md).

## Phase 0 — scaffold (done)

- [x] Submodules: `vendor/openhuman`, `vendor/tinyrsi`, `benchmarks`.
- [x] `ohrsi` crate: campaign schema + validation, workspace checks, `check` / `list`.
- [x] Draft campaigns `memory-recall-pack`, `memory-temporal-recall`.
- [x] Docs, ADRs, CI without submodules.

## Phase 1 — benchmark adapter

- [ ] Decide the candidate-to-benchmark binding (spec open question 1); record an ADR.
- [ ] `scripts/bench/memory-metric.sh`: run `benchmarks/memory` for one arm on one
      seed against a given OpenHuman checkout, print `METRIC memory_accuracy=…`.
      Test: offline fixture `answers.jsonl` → expected metric.
- [ ] `ohrsi check` verifies the campaign command exists.

## Phase 2 — candidates and gate

- [ ] `ohrsi` creates/removes OpenHuman worktrees under `runs/`.
- [ ] Surface guard: diff a candidate against its base, reject frozen/unlisted changes.
- [ ] Build-and-test gate per campaign.

## Phase 3 — loop

- [ ] Depend on `vendor/tinyrsi` (`tinyrsi` crate) and map `Campaign` → `ScriptSubject`.
- [ ] `ohrsi run <campaign>` with JSONL trace and ledger under `runs/`.
- [ ] Offline end-to-end test with a mock model and mock benchmark.

## Phase 4 — ship

- [ ] `ohrsi export <run>`: incumbent diff + evidence summary for an upstream PR.
- [ ] First live campaign within its cap; commit the report under `docs/results/`.

## Verification

The four contract commands in `AGENTS.md`, plus `scripts/bootstrap.sh` on a fresh clone.
