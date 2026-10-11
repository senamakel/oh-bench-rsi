# `lme2-recall` loop

Improve OpenHuman's recall over long agent histories, measured on the frozen
LongMemEval-V2 web subset. This folder is a
[tinyrsi loop folder](../../vendor/tinyrsi/docs/specs/rsi-cli.md): the
`tinyrsi` binary runs it, and nothing else in this repository drives the loop.

```sh
cargo install --path vendor/tinyrsi/crates/tinyrsi-cli
scripts/bootstrap.sh                 # submodules, TinyMemory's local rsi/main
tinyrsi check loops/lme2-recall      # prints `ok: lme2-recall ...`
tinyrsi run   loops/lme2-recall      # paid: needs OPENROUTER_API_KEY and Docker
```

`tinyrsi run` serves its UI on `http://127.0.0.1:8787` while it works and
stores the run in `~/.tinyrsi/runs/lme2-recall/<run-id>/` (`$TINYRSI_HOME`
moves it). `tinyrsi runs` and `tinyrsi show lme2-recall/<run-id>` read it back.

## What is in the folder

| Path | Role |
| --- | --- |
| `rsi.json` | The loop: subject, hive, scorer, decision, gates, budget, epochs |
| `brief.md` | The epoch brief template; tinyrsi fills `{{incumbent}}`, `{{history}}`, `{{failures}}`, `{{ideas}}` |
| `hive/` | The swarm (lead + implementer) in the tinyhivemind hive language; see [`hive/README.md`](hive/README.md) |
| `scorers/lme2.py` | The frozen measurement; writes `result.json` |
| `gates/leakage.py` | Rejects a diff carrying subset questions, gold answer fragments or benchmark canaries |
| `gates/green.py` | `cargo test -p <crate>` for every TinyMemory crate the candidate touches |
| `tests/` | `python3 -m unittest discover -s loops/lme2-recall/tests` |

## One epoch

1. tinyrsi scores the incumbent (TinyMemory's local `rsi/main`) once per
   commit and caches it.
2. It renders `brief.md` from the incumbent's `result.json`: dev metrics and
   up to ten dev failures (two per question type), each with the question,
   the model's answer and the recall pack — never a gold answer.
3. The hive runs; `implementer-a` edits its own TinyMemory worktree on
   `rsi/cand-<run>-e<n>-<seat>`.
4. For each changed candidate: the surface check (`subject.editable` and
   `frozen`, and `require_test_change`), then `gates/leakage.py`, then
   `gates/green.py`, then — if `score_reserve_usd` is still affordable — the
   scorer.
5. The decision is an exact one-sided sign test at α = 0.10 on the 150 test
   questions' paired 0/1 scores (`splits.test.items`). An accepted candidate
   fast-forwards the local `rsi/main`. Nothing is pushed.

## The scorer's contract

`scorers/lme2.py` reads `RSI_CANDIDATE` (a TinyMemory worktree root),
`RSI_OUT`, `RSI_GATEWAY` and `RSI_LABEL`. It builds `adapters/lme2-memd`
against `$RSI_CANDIDATE/crates`, runs CortexDB and the unmodified official
harness in Docker with every model call tagged through the gateway
(`/__tag/ingest/cortex`, `/__tag/reader/harness`, `/__tag/judge/harness`),
reuses `runs/ingest-cache/<key>` when the TinyMemory write path is unchanged
(a restored ingest that still had enrichment queued is re-cached once it has
settled, so the queue is paid for once), and writes into `$RSI_OUT`:

- `result.json` — `score` (dev accuracy), `splits.dev.items` and
  `splits.test.items` (per-question 0/1), `failures` (dev only) and `metrics`
  (`dev_accuracy`, `test_accuracy`, `dev_pack_tokens_mean`,
  `dev_accuracy_<type>`);
- `summary.json`, `dev_failures.jsonl`, `eval.log`, `harness.log`,
  `memd.log`, `cortex.log`;
- on failure, `cortex-data/`, which `scripts/bench/lme2_snapshot.py` can
  salvage into the ingest cache.

It also prints `METRIC lme2_dev_accuracy=…` and `METRIC lme2_test_accuracy=…`.

## Tuning with `env`

Step commands see only `PATH`, `HOME`, `TMPDIR`, the `RSI_*` variables and
`rsi.json`'s `env` map. The committed `rsi.json` sets no `env`: a full run
uses the frozen defaults. For a local smoke run, add an `env` map with any of:

| Key | Default | Effect |
| --- | --- | --- |
| `LME2_LIMIT` | unset (all 225) | Score only the first N questions of each split |
| `LME2_ENGINE` | `cortex` | `reference` uses tinymemory-api's in-memory conformance engine (no CortexDB) |
| `LME2_CONCURRENCY` | `8` | Reader concurrency in the harness |
| `MEMD_SETTLE_SECS` | `10800` | How long memd waits for CortexDB enrichment to settle |
| `LME2_MOUNT_ROOT` | the primary checkout | Path mounted into the harness container |

A limited or `reference` run is not comparable with a full one; do not
commit an `env` that changes the measurement.

## Manual scoring

```sh
python3 -I loops/lme2-recall/scorers/lme2.py \
  --candidate vendor/openhuman/vendor/tinymemory --out runs/manual \
  --gateway http://127.0.0.1:<port> --limit 5
```

Any OpenAI-compatible base that accepts the `/__tag/<phase>/<seat>/v1` paths
works as `--gateway`; through anything other than a tinyrsi gateway the
calls are not capped. `tinyrsi score loops/lme2-recall --label <name>` runs
the scorer under the real gateway instead.
