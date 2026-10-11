# 7. Run the loop as a tinyrsi loop folder

- **Status:** Accepted
- **Date:** 2026-10-11
- **Supersedes:** the implementation parts of ADR 0002 (CI builds `ohrsi`
  without submodules) and ADR 0005 (`crates/ohrsi-gateway` is the gateway).
  Their decisions — pinned submodules, one capped gateway as the only source
  of spend — stand.

## Context

This repository hand-wrote every generic part of its RSI loop: the epoch
driver, gate and sign test in Python (`scripts/rsi/`), the swarm host that
ran the hive package as OpenHuman agents (`swarm/`), a spend-capped
OpenRouter gateway (`crates/ohrsi-gateway`), a campaign manifest validator
(`crates/ohrsi`), and a copy of tinyrsi's trace schema with placeholder
values for rsi-viz. Only the benchmark — the LongMemEval-V2 measurement, its
frozen subset and the leakage rules — was specific to OpenHuman memory.

tinyrsi now ships a `tinyrsi` binary that runs a loop defined as a folder:
`rsi.json`, a brief template, a tinyhivemind hive package and step scripts.
It owns the epoch loop, the swarm on OpenHuman agents, the spend gateway,
candidate git worktrees with auto-commit, the editable/frozen surface check
and `require_test_change`, the incumbent score cache, the brief, the exact
sign test, fast-forward promotion of a branch, the trace, ledger and run
store, and the UI.

## Decision

The loop is the loop folder `loops/lme2-recall/`, run with
`tinyrsi run loops/lme2-recall` from `vendor/tinyrsi`
(`cargo install --path vendor/tinyrsi/crates/tinyrsi-cli`). This repository
keeps only:

- the scorer (`scorers/lme2.py`, formerly `scripts/bench/lme2_eval.py`),
  with its measurement unchanged, writing tinyrsi's `result.json`;
- two gates: leakage (`gates/leakage.py`) and TinyMemory's own tests
  (`gates/green.py`);
- the hive package (`hive/`, formerly `hives/memory-rsi`) and the brief
  template (`brief.md`, formerly `scripts/rsi/brief.py`);
- the memd adapter, the harness backend and the frozen subset.

The subject is the vendored TinyMemory repository itself (a `git` subject on
local `rsi/main`), so surface paths are relative to TinyMemory, and runs are
stored in `~/.tinyrsi` instead of `runs/`. The ingest cache stays in
`runs/ingest-cache/`. The `ohrsi`, `ohrsi-gateway` and `swarm` crates, the
root Cargo workspace, `scripts/rsi/`, `with-gateway.sh` and the campaign TOML
are deleted. CI runs the scripts' unit tests, `tinyrsi check` and the memd
build.

## Consequences

- A loop change is a JSON, Markdown or Python change; there is no Rust to
  maintain here beyond the adapter.
- Fixes to the generic machinery land upstream in tinyrsi and arrive with a
  gitlink bump; this repository depends on tinyhumansai/tinyrsi#50 and #51.
- CI must check out `vendor/tinyrsi` and `vendor/openhuman` recursively and
  builds the binary, so it is slower than the old submodule-free CI.
- Run history from before the move (`runs/<run>/`, `docs/results/run-00`
  to `run-02`) uses the old layout; later runs live in the tinyrsi run store.
- The rsi-viz Swarm view patches in `patches/tinyrsi/` target the old pin and
  the old trace layout; the binary's own UI replaces them.
