# Roadmap

## Shipped

- Submodules: OpenHuman, tinyrsi and openhuman-benchmarks, pinned.
- LongMemEval-V2 (web, small tier) measurement: the `lme2-memd` TinyMemory adapter, the
  official harness's `oh_tinymemory` backend, the frozen 75 dev / 150 test subset and the
  ingest cache.
- Baseline run (`run-02`): dev 0.320, test 0.160.
- The loop as a tinyrsi loop folder, `loops/lme2-recall`, run by the `tinyrsi` binary
  (ADR 0007). It replaced this repository's own epoch driver, gate, decision, swarm host
  and spend gateway.

## Next

See [`docs/plans/rsi-memory-loop.md`](docs/plans/rsi-memory-loop.md):

1. Measure the `recall::excerpt` candidate in `patches/tinymemory/` through the loop.
2. First full `tinyrsi run loops/lme2-recall` within its $5 cap; commit the report under
   `docs/results/`.
3. Grow the hive roster (analyst, researcher) once the two-seat swarm proves itself.
4. Upstream promoted TinyMemory changes, each with its regression test.

## Out of scope

- Editing benchmarks, datasets, judges or scorers to raise a score.
- Unbounded paid runs: every run carries a hard spend cap.
