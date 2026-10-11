# Run reports

One file per run of the $50 campaign (10 runs, $5 cap each). Ledger values
for run-00 to run-02 come from `runs/<run>/budget.json` and
`runs/ledger-total.json`; later runs are `tinyrsi run loops/lme2-recall` runs,
whose ledger is `~/.tinyrsi/runs/lme2-recall/<run-id>/run.json` and
`~/.tinyrsi/spend.json` (ADR 0007). "Charged" includes reservations kept
because a call's cost was unknown.

| Run | Purpose | Charged | Outcome |
| --- | --- | ---: | --- |
| [run-00](run-00.md) | gateway/adapter probes | $0.96* | wiring verified; *orphaned reservations, see report |
| [run-01](run-01.md) | Claude test run: setup, swarm smoke, first ingests | $1.11 | swarm wiring proven; two ingests lost to config bugs |
| [run-02](run-02.md) | baseline ingest + eval | $2.54 | **dev 0.320 · test 0.160**; 90% of dev failures have no answer in the pack |

Campaign charged so far: **$4.66 of $50** (settled $3.67, uncertain $0.99).
