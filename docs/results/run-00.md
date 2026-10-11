# run-00 — probes

- Smoke eval, reference engine, 4 questions: **$0.00076**. Proved the
  container → loopback gateway/memd path and the official harness runner.
- CortexDB ingest-cost probe (5 trajectories): exposed two gateway issues.
  1. Concurrency × the 8k-token default reservation saturated the $1 probe
     cap, so CortexDB's calls were refused (657 denials).
  2. **Bug:** killing the client mid-call dropped axum's handler future, so
     48 reservations ($0.96) were never settled. They became "uncertain" in
     the campaign ledger on restart, conservatively charged. Fixed by running
     every forward on a detached task; regression test
     `a_client_that_disconnects_mid_call_still_settles`.
- GLM reasoning on every enrichment call (~2.5k completion tokens each)
  motivated pinning `reasoning.effort = low` by default in the gateway.
- Second probe (2 trajectories, 7 items): $0.0061 → ~$0.0009/item at low
  reasoning before CortexDB's background roles were accounted for.
