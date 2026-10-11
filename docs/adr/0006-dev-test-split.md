# 6. The swarm learns on dev; promotion is decided on test

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

A proposer that sees per-question failures can overfit them, and one that
sees gold answers can encode them. A loop that both learns from and is judged
on the same questions would report its own overfitting as improvement.

## Decision

The frozen subset is split by seeded stratification into 75 dev and 150 test
questions. The swarm's brief carries dev failures (question, model answer,
retrieved pack) but never gold answers or anything from test. Promotion is a
paired sign test on test only. The gate rejects diffs containing subset
question text, gold answer phrases or benchmark site names.

## Consequences

- Test accuracy is the honest progress signal; dev accuracy can rise alone.
- With 150 questions, small true gains can read as inconclusive; that is
  preferable to promoting noise.
