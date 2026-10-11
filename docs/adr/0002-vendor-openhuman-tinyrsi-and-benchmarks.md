# 2. Vendor OpenHuman, tinyrsi and the benchmarks as submodules

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

The loop needs an exact OpenHuman to patch, an exact loop library, and an exact
measurement. Floating any of them makes a before/after comparison meaningless.

## Decision

Pin `vendor/openhuman`, `vendor/tinyrsi` and `benchmarks` (openhuman-benchmarks)
as git submodules. Every run records the three gitlinks. Changes to any of them
land upstream first, then a gitlink bump here, which invalidates prior baselines.

This repository was generated from the TinyBus module template; it is not a
TinyBus module, so the template's contract crate, `cdylib`, release workflow and
`vendor/tinybus` were removed.

## Consequences

- Reproducible runs; a gitlink bump is an explicit, reviewable event.
- A full checkout is large (OpenHuman and its nested vendors); CI therefore
  builds the `ohrsi` crate without submodules.
