# 5. The spend gateway is the only path to a model

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

A run spends through four clients — swarm agents, CortexDB enrichment and
embeddings, the benchmark reader and its judge — each with its own idea of
cost, retries and concurrency. A cap enforced in any one of them is not a cap.

## Decision

`crates/ohrsi-gateway` holds the only provider key and fronts OpenRouter on
loopback. It reserves each call's worst-case cost against both the run and
campaign ledgers before forwarding (HTTP 402 when it does not fit), settles
to the reported cost, keeps the reservation when cost is unknown, settles
even when the client disconnects, halts on a price-bound breach, and tags
every call with its phase and seat for attribution.

## Consequences

- The $5/run and $50/campaign limits hold regardless of client behaviour.
- Conservative accounting can over-charge (orphaned or unknown-cost calls
  stay charged); that is the intended failure direction.
