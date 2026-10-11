# RSI memory loop

- **Status:** Draft
- **Owner:** oh-bench-rsi maintainers

## Problem

OpenHuman's memory arm trails full context on the pinned memory audit
(see [`../memory-benchmarks.md`](../memory-benchmarks.md)). The known failures
are in recall gathering, pack rendering and temporal ranking — code with many
plausible tweaks and a slow, noisy, paid measurement. A disciplined automated
loop can search that space faster than hand iteration, provided it cannot cheat.

## Goals

- Run tinyrsi's improvement loop against OpenHuman memory code, one campaign at
  a time, with the benchmark as the only judge.
- Accept a candidate only on paired-seed evidence with a positive lower bound.
- Produce, for every accepted candidate, a patch that applies to upstream
  OpenHuman/TinyMemory with its tests green.
- Keep every run's ledger, trace and per-seed metrics under `runs/<campaign>/<run-id>/`.

## Non-goals

- Changing benchmarks, data, judges or scoring.
- Automatic merging upstream: a human reviews every PR.
- Desktop end-to-end memory evaluation (a later suite).

## Proposed behavior

1. **Campaign** — `campaigns/<name>.toml`, format in [`campaigns.md`](campaigns.md).
2. **Candidate checkout** — each candidate is a git worktree of
   `vendor/openhuman` (with its nested TinyMemory) under
   `runs/<campaign>/<run-id>/candidates/<id>/`. Proposals replace whole files on
   the editable surface only.
3. **Gate** — before any paid evaluation, the candidate must build and pass the
   affected crates' tests (`cargo test -p tinymemory-tools`, plus
   `openhuman-core` memory tests when OH core is on the surface).
4. **Measure** — the campaign command runs the frozen suite against the
   candidate checkout once per seed, with `TINYRSI_SEED` set, and prints
   `METRIC <name>=<value>`.
5. **Decide** — tinyrsi's paired comparison against the incumbent; promote when
   the lower bound of the improvement exceeds `0` (or the configured margin).
6. **Stop** — on target, `max_epochs`, patience, or `usd_cap`.
7. **Export** — `ohrsi export <run>` writes the incumbent's diff against the
   pinned base, ready for an upstream PR.

## Invariants

- No path outside `surface.editable` differs between a candidate and its base.
- `benchmarks/` is byte-identical before and after a run.
- The proposer never sees gold answers, judge rationales, or question metadata
  beyond the per-seed aggregate metric and stderr tails.
- Total charged spend never exceeds `usd_cap`.

## Acceptance criteria

- One campaign runs end to end offline against a mock model and mock benchmark.
- One live campaign produces a ledger with at least one decided candidate, its
  spend under cap, and an exportable diff.
- A tampered candidate (touches a frozen path) is rejected before evaluation.

## Open questions

- How to point `benchmarks/memory`'s Cargo path dependencies at a candidate
  checkout without editing the submodule (copied manifest in this repo vs.
  an upstream `OPENHUMAN_DIR`-style override).
- Seed semantics: the audit pins seed 27 for task selection; per-seed variance
  probably has to come from answer sampling or task resampling instead.
- Whether a held-out question set is needed to guard against overfitting the
  20-question subset (likely yes before claiming a gain).
