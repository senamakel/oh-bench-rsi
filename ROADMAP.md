# Roadmap

## Shipped

- Repository scaffold: OpenHuman, tinyrsi and openhuman-benchmarks pinned as
  submodules; `ohrsi` crate with validated campaign manifests and
  `ohrsi check` / `ohrsi list`.
- Two draft campaigns from the memory audit findings: `memory-recall-pack` and
  `memory-temporal-recall`.

## Next

See [`docs/plans/rsi-memory-loop.md`](docs/plans/rsi-memory-loop.md):

1. `scripts/bench/memory-metric.sh` — run the frozen memory suite against a
   candidate checkout and print `METRIC memory_accuracy=<value>`.
2. Candidate checkouts: OpenHuman git worktrees under `runs/`.
3. Link tinyrsi's script loop and drive one campaign end to end.
4. Promote accepted candidates as upstream PRs to OpenHuman / TinyMemory.
5. Extend to LongMemEval-V2 once an OH `Memory.insert/query` adapter exists.

## Out of scope

- Editing benchmarks, datasets, judges or scorers to raise a score.
- Unbounded paid runs: every campaign carries a hard spend cap.
