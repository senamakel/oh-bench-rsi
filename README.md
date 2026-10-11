# oh-bench-rsi

Recursive self-improvement (RSI) for [OpenHuman](https://github.com/tinyhumansai/openhuman)'s
memory, measured by the pinned memory benchmarks in
[openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks) and driven by
[tinyrsi](https://github.com/tinyhumansai/tinyrsi).

The loop: a hive of OpenHuman agents proposes a change to TinyMemory's recall code, the
**frozen** benchmark measures the candidate against the incumbent on the same questions, and
the candidate is kept only if a paired sign test on held-out questions says it is better, all
within a hard spend cap. Promoted candidates stay on local branches; a human upstreams them.

> Status: the loop runs on **LongMemEval-V2** (web subset) as a tinyrsi loop folder,
> [`loops/lme2-recall`](loops/lme2-recall/README.md). tinyrsi supplies the epoch loop, the
> swarm, the spend gateway, candidate worktrees, the decision, promotion, the run store and
> the UI; this repository supplies the benchmark. Results per run are in
> [`docs/results/`](docs/results/).

## One RSI run

```sh
scripts/bootstrap.sh                                     # submodules, rsi/main, check
cargo install --path vendor/tinyrsi/crates/tinyrsi-cli   # the `tinyrsi` binary
export OPENROUTER_API_KEY=...                            # or loops/lme2-recall/.env
tinyrsi run loops/lme2-recall                            # UI on http://127.0.0.1:8787
```

Runs are stored in `~/.tinyrsi` (set `TINYRSI_HOME` to move them); `tinyrsi runs` lists
them and `tinyrsi serve` reopens the UI. A run needs Docker and a populated
`benchmarks/.cache` (see [`docs/memory-benchmarks.md`](docs/memory-benchmarks.md)).

## Layout

See the project structure in [`AGENTS.md`](AGENTS.md#project-structure): the loop folder
under `loops/`, the LongMemEval-V2 adapter under `adapters/`, the frozen subset under
`campaigns/lme2/`, and the pinned OpenHuman, tinyrsi and benchmarks submodules.

## Ground rules

- `benchmarks/` is frozen, and so is the scorer's measurement. Winning by changing the
  measurement is not a win.
- A proposal may only edit TinyMemory's `tinymemory-tools` and `tinymemory-integrations`
  and must change a test; tinyrsi rejects anything else before scoring.
- No gold answer or test-split detail reaches the hive; a leakage gate rejects diffs that
  carry benchmark content.
- Every run is capped (`budget.run_usd`, `budget.total_usd` in `rsi.json`).

## Documentation

- [`AGENTS.md`](AGENTS.md) — how humans and agents work here (`CLAUDE.md` links to it)
- [`loops/lme2-recall/README.md`](loops/lme2-recall/README.md) — the loop folder
- [`docs/memory-benchmarks.md`](docs/memory-benchmarks.md) — the benchmarks, baselines and known failures
- [`docs/specs/rsi-memory-loop.md`](docs/specs/rsi-memory-loop.md) — the loop's contract
- [`docs/plans/rsi-memory-loop.md`](docs/plans/rsi-memory-loop.md) — delivery plan
- [`docs/adr/`](docs/adr/) — architecture decisions

## Development

```sh
python3 -m unittest discover -s loops/lme2-recall/tests
OPENROUTER_API_KEY=placeholder cargo run --manifest-path vendor/tinyrsi/Cargo.toml \
  -p tinyrsi-cli -- check loops/lme2-recall
cargo build --manifest-path adapters/lme2-memd/Cargo.toml --target-dir target/memd
```

CI runs these with `--locked`, checking out `vendor/tinyrsi` and `vendor/openhuman`
recursively. None of them spends money.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
