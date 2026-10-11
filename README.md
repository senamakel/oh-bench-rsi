# oh-bench-rsi

Recursive self-improvement (RSI) for [OpenHuman](https://github.com/tinyhumansai/openhuman)'s
memory, measured by the pinned memory benchmarks in
[openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks) and driven by
[tinyrsi](https://github.com/tinyhumansai/tinyrsi).

The loop: propose a patch to OpenHuman's memory code, run the **frozen** benchmark on the
patched checkout over paired seeds, keep the candidate only if it beats the incumbent with
a positive confidence bound, and repeat within a hard spend cap. Accepted candidates become
upstream pull requests to OpenHuman (or its vendored TinyMemory), never local forks.

> Status: the loop runs end to end on **LongMemEval-V2** (web subset): a
> spend-capped gateway, an OpenHuman TinyMemory adapter for the official
> harness, a gate, a paired decision, and a swarm of GLM 5.3 Flash OpenHuman
> agents defined in the tinyhivemind hive language
> ([`hives/memory-rsi`](hives/memory-rsi/README.md)), visualized live in
> rsi-viz. Results per run are in [`docs/results/`](docs/results/).

## Layout

See the project structure in [`AGENTS.md`](AGENTS.md#project-structure): the
`ohrsi` campaign crate and the `ohrsi-gateway` spend gateway under `crates/`,
the `swarm/` host, the `hives/memory-rsi` package, the LongMemEval-V2 adapter
under `adapters/`, and the run driver under `scripts/rsi/`.

## One RSI run

```sh
cargo build --manifest-path swarm/Cargo.toml --target-dir target/swarm
scripts/bench/with-gateway.sh run-02 5 -- python3 scripts/rsi/epoch.py --run run-02
# watch it: cd vendor/tinyrsi/apps/rsi-viz && RSI_RUN_ROOT=../../../../runs npm run dev
```

## Quick start

```sh
git clone --recurse-submodules=benchmarks --recurse-submodules=vendor <url>   # or:
scripts/bootstrap.sh          # init submodules, then `ohrsi check`
cargo run -p ohrsi -- list    # show campaigns
cargo run -p ohrsi -- check   # validate every campaigns/*.toml
```

## Ground rules

- `benchmarks/` is frozen. A campaign may only edit paths under `vendor/openhuman/`, and the
  manifest validator rejects anything else. Winning by changing the measurement is not a win.
- Every campaign has a `usd_cap`. Read the benchmark's budget ledger before spending.
- A candidate counts only if it improves on paired seeds and keeps OpenHuman's tests green.

## Documentation

- [`AGENTS.md`](AGENTS.md) — how humans and agents work here (`CLAUDE.md` links to it)
- [`docs/memory-benchmarks.md`](docs/memory-benchmarks.md) — the benchmarks, baselines and known failures
- [`docs/specs/rsi-memory-loop.md`](docs/specs/rsi-memory-loop.md) — the loop's contract
- [`docs/specs/campaigns.md`](docs/specs/campaigns.md) — the campaign manifest format
- [`docs/plans/rsi-memory-loop.md`](docs/plans/rsi-memory-loop.md) — delivery plan
- [`docs/adr/`](docs/adr/) — architecture decisions

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

CI runs exactly these and does not check out submodules: the `ohrsi` crate builds without them.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
