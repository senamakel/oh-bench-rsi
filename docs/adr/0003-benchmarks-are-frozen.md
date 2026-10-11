# 3. Benchmarks are frozen; only OpenHuman code is editable

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

An optimizer rewarded by a score will find the cheapest way to raise it,
including changing the scorer, the questions, or the budget.

## Decision

Campaign surfaces may only name paths under `vendor/openhuman/`. `benchmarks/`
is frozen in every campaign, enforced by `Campaign::validate`, and campaigns may
freeze more (for example TinyMemory's public API). Benchmark changes go upstream
as their own reviewed PRs.

## Consequences

- A score gain is attributable to OpenHuman code.
- Some real improvements (better answer formatting in the harness) are out of
  reach of the loop by design and must be argued for upstream.
