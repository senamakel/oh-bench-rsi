# oh-bench-rsi

Recursive self-improvement (RSI) for [OpenHuman](https://github.com/tinyhumansai/openhuman)'s
memory, measured by the pinned memory benchmarks in
[openhuman-benchmarks](https://github.com/tinyhumansai/openhuman-benchmarks) and driven by
[tinyrsi](https://github.com/tinyhumansai/tinyrsi).

The loop: propose a patch to OpenHuman's memory code, run the **frozen** benchmark on the
patched checkout over paired seeds, keep the candidate only if it beats the incumbent with
a positive confidence bound, and repeat within a hard spend cap. Accepted candidates become
upstream pull requests to OpenHuman (or its vendored TinyMemory), never local forks.

> Status: scaffold. Campaign manifests validate; the benchmark adapter and the loop wiring
> are the next phases in [`docs/plans/rsi-memory-loop.md`](docs/plans/rsi-memory-loop.md).

## Layout

```text
Cargo.toml            # virtual workspace
crates/ohrsi/         # the RSI driver: campaigns, workspace checks, `ohrsi` CLI
campaigns/            # one TOML manifest per improvement campaign
scripts/bootstrap.sh  # initialize the submodules this repo needs
runs/                 # git-ignored run output (candidates, ledgers, traces)
vendor/openhuman/     # the code being improved (submodule)
vendor/tinyrsi/       # the improvement loop library (submodule)
benchmarks/           # openhuman-benchmarks — frozen measurement (submodule)
docs/                 # specs, plans, ADRs, benchmark notes
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
