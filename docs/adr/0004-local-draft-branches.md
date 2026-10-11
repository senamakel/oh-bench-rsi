# 4. Candidate changes live on local draft branches

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

The swarm and Claude edit OpenHuman's TinyMemory continuously. Pushing or
merging those changes upstream automatically would put unreviewed,
benchmark-motivated code into products other people use.

## Decision

Every candidate is a local git worktree on `rsi/cand-*` branched from a local
`rsi/main` in the vendored repository. Promotion is a local fast-forward of
`rsi/main`. Nothing is pushed, merged upstream, or opened as a pull request
by the harness; a human upstreams promoted changes from the run reports.

## Consequences

- The vendored submodules run ahead of their pinned gitlinks during a
  campaign; this repository never commits those gitlinks mid-campaign.
- Every promoted diff is reviewable as a branch with its test and its paired
  evidence before anyone upstreams it.
