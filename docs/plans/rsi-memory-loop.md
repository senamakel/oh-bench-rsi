# Plan: RSI memory loop

Spec: [`../specs/rsi-memory-loop.md`](../specs/rsi-memory-loop.md).

## Phase 0 — scaffold (done)

- [x] Submodules: `vendor/openhuman`, `vendor/tinyrsi`, `benchmarks`.
- [x] Docs, ADRs, CI.

## Phase 1 — benchmark adapter (done)

- [x] `adapters/lme2-memd`: TinyMemory over loopback HTTP, built per candidate.
- [x] `adapters/lme2`: the official harness's `oh_tinymemory` backend.
- [x] Frozen subset `campaigns/lme2/subset.json` (75 dev / 150 test, ADR 0006).
- [x] Ingest cache keyed on the write path; `lme2_snapshot.py` salvages a
      failed ingest.

## Phase 2 — hand-written loop (done, retired)

- [x] Epoch driver, gate, sign test and brief in `scripts/rsi/`; swarm host in
      `swarm/`; spend gateway in `crates/ohrsi-gateway`.
- [x] Baseline run (`run-02`).

## Phase 3 — tinyrsi loop folder (ADR 0007)

- [x] Bump `vendor/tinyrsi` to the `tinyrsi` binary (tinyrsi#50, #51).
- [x] `loops/lme2-recall/`: `rsi.json`, `brief.md`, `hive/`,
      `scorers/lme2.py` (writes `result.json`), `gates/leakage.py`,
      `gates/green.py`, unit tests in `tests/`.
- [x] Delete `scripts/rsi/`, `with-gateway.sh`, `swarm/`, `crates/`, the root
      Cargo workspace and the campaign TOML.
- [x] CI: unit tests, `tinyrsi check loops/lme2-recall`, memd build.
- [ ] First `tinyrsi run loops/lme2-recall` within its $5 cap: confirm the
      scorer under the step contract (Docker mounts of the run store, gateway
      tags, ingest cache hit) and commit the report under `docs/results/`.

## Phase 4 — ship

- [ ] Measure the `recall::excerpt` candidate (`patches/tinymemory/`).
- [ ] Upstream promoted TinyMemory changes from `patches/`, each with its test.

## Verification

```sh
python3 -m unittest discover -s loops/lme2-recall/tests
OPENROUTER_API_KEY=placeholder cargo run --manifest-path vendor/tinyrsi/Cargo.toml \
  -p tinyrsi-cli -- check loops/lme2-recall
cargo build --manifest-path adapters/lme2-memd/Cargo.toml --target-dir target/memd
```
