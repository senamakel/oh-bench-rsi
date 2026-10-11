# Repository Guidelines

This file is the single source of truth for how humans and coding agents work
in this repository. `CLAUDE.md` is a symlink to this file, so every agent reads
the same instructions.

Keep it current: when the layout, commands, or rules change, update this file
in the same commit.

## Mission

Make OpenHuman ace its memory benchmarks through recursive self-improvement:
an automated loop proposes changes to OpenHuman's memory code, measures them on
frozen benchmarks, and keeps only verified improvements. The loop is a
[tinyrsi](https://github.com/tinyhumansai/tinyrsi) **loop folder**,
[`loops/lme2-recall/`](loops/lme2-recall/README.md), run by the `tinyrsi`
binary. Read [`docs/memory-benchmarks.md`](docs/memory-benchmarks.md) and
[`docs/specs/rsi-memory-loop.md`](docs/specs/rsi-memory-loop.md) before working
on the loop.

## RSI Rules

These are non-negotiable for humans, agents, and the loop's own proposer.

1. **The measurement is frozen.** Never edit `benchmarks/` — datasets, task
   selection, prompts, judges, scorers, budgets, or the gateway — to raise a
   score. Benchmark changes go upstream to openhuman-benchmarks as their own,
   separately reviewed PR, and reset every baseline.
2. **Edit only the declared surface.** The loop's `subject.editable` prefixes
   in `loops/lme2-recall/rsi.json` (TinyMemory's `tinymemory-tools` and
   `tinymemory-integrations`; `tinymemory-api` is frozen) are the only files a
   proposal may change. tinyrsi's surface check enforces this before any
   score is spent; do not widen the surface or weaken the gates to pass a
   candidate.
3. **No leakage.** Gold answers, judge outputs, and private question metadata
   never reach the proposer or the system under test. Do not hard-code
   benchmark questions, entities, or answers into OpenHuman.
4. **Paired, repeated evidence.** Compare candidate and incumbent on the same
   seeds. One lucky run is not an improvement; report inconclusive results as
   inconclusive.
5. **Spend is capped.** Every model call goes through the run's tinyrsi spend
   gateway, capped by the loop's `budget.run_usd` and `budget.total_usd`.
   Read the spend ledger (`$TINYRSI_HOME/spend.json`) before a paid run, and
   never loop on paid calls unattended without a cap.
6. **Stay local.** Candidates and promotions live on local `rsi/*` branches
   in the vendored repositories (ADR 0004). tinyrsi never pushes, merges,
   or opens pull requests; a human upstreams promoted diffs, each with its
   regression test, from the run reports. Never commit vendored code into
   this repository's history.
7. **Keep OpenHuman green.** A candidate that fails OpenHuman's or
   TinyMemory's own tests is rejected regardless of its score.

## Project Structure

```text
loops/lme2-recall/      # the tinyrsi loop folder: the whole RSI loop
├── rsi.json            # subject, hive, scorer, decision, gates, budget
├── brief.md            # the epoch brief template for the lead seat
├── hive/               # the swarm, in the tinyhivemind hive language
├── scorers/lme2.py     # frozen LongMemEval-V2 measurement → result.json
├── gates/              # leakage.py, green.py: checked before any score spend
└── tests/              # stdlib unittest for the pure parts of the scripts
adapters/
├── lme2-memd/          # standalone crate: OpenHuman TinyMemory over loopback HTTP,
│                       # built per candidate against its TinyMemory checkout
└── lme2/               # the official harness's `Memory` backend + runner
scripts/
├── bootstrap.sh        # submodules, local `rsi/main`, `tinyrsi check`
└── bench/              # lme2_subset.py, lme2_snapshot.py
campaigns/lme2/         # subset.json: the frozen 75 dev / 150 test subset
patches/                # local draft patches exported from vendored `rsi/*` branches
research/               # read-only notes on TinyMemory, OpenHuman memory, LongMemEval-V2
runs/                   # git-ignored: ingest cache
vendor/openhuman/       # pinned OpenHuman (TinyMemory at vendor/tinymemory)
vendor/tinyrsi/         # pinned tinyrsi: the `tinyrsi` binary (crates/tinyrsi-cli)
benchmarks/             # pinned openhuman-benchmarks — frozen measurement
docs/                   # memory-benchmarks.md, specs/, plans/, adr/, results/
```

tinyrsi owns the epoch loop, the hive swarm on OpenHuman agents, the spend
gateway, candidate worktrees and the surface check, the incumbent score
cache, the brief, the sign test, promotion, the trace and run store, and the
UI. This repository owns only the benchmark: the loop folder, the memd
adapter and the frozen subset.

### Running

```sh
scripts/bootstrap.sh                                         # submodules, rsi/main, check
cargo install --path vendor/tinyrsi/crates/tinyrsi-cli       # the `tinyrsi` binary
tinyrsi check loops/lme2-recall
tinyrsi run   loops/lme2-recall      # UI on http://127.0.0.1:8787
tinyrsi runs                         # every run; `tinyrsi show lme2-recall/<id>`
```

`tinyrsi run` runs every chat call on the local LLM ladder router's `flash`
rung: start it as `TINYRSI_UPSTREAM=http://127.0.0.1:6969/v1 tinyrsi run
loops/lme2-recall`. It needs `LADDER_API_KEY` (the gateway's key) and
`OPENROUTER_API_KEY` (CortexDB's embeddings only) in the environment or in
`loops/lme2-recall/.env`, Docker, and a populated `benchmarks/.cache`. Runs are
stored in `~/.tinyrsi` (or `$TINYRSI_HOME`); the ingest cache stays in
`runs/ingest-cache/`. The LongMemEval-V2 harness's lockfile is Linux-only; it
runs in the `ghcr.io/astral-sh/uv:python3.11-bookworm` container, which
reaches the host's loopback gateway and memd as `host.docker.internal`.

## Build And Test

Run every command from the repository root. CI runs these:

```sh
python3 -m unittest discover -s loops/lme2-recall/tests
OPENROUTER_API_KEY=placeholder LADDER_API_KEY=placeholder cargo run --locked --manifest-path vendor/tinyrsi/Cargo.toml \
  -p tinyrsi-cli -- check loops/lme2-recall
cargo build --locked --manifest-path adapters/lme2-memd/Cargo.toml --target-dir target/memd
```

`check` must print `ok: lme2-recall ...`. It only needs the key's *name* to
be set; it never reads the value or calls a model. Never run the paid loop or
the Docker evaluation as a check.

Never skip, ignore, or delete a failing test to make a command pass. Fix the
root cause, or stop and report the blocker.

## Coding Style

Loop scripts are Python 3, standard library only, run with `python3 -I`. Keep
their pure parts (parsing, result building, matching) in functions the unit
tests import, and the subprocess and Docker plumbing thin.

The memd adapter is Rust: use standard `rustfmt` output and Rust 2024 idioms. Do not hand-format around
`rustfmt`, and do not add `#[rustfmt::skip]` without a comment explaining why.

- `snake_case` for modules, files, functions, methods, fields, and locals.
- `PascalCase` for types, traits, and enum variants; `SCREAMING_SNAKE_CASE` for
  constants and statics.
- Name things for what they are, not for their layer: `RetryPolicy`, not
  `RetryHelper`.
- Prefer small, typed APIs over stringly-typed ones. Accept `&str` and generic
  `impl Into<String>` at boundaries; return owned, concrete types.
- Keep the public surface minimal: default to private, and export deliberately
  from `src/lib.rs`.
- Do not write `unsafe`. If the adapter genuinely needs it, document every
  invariant with a `// SAFETY:` comment in its own reviewed commit.

### Errors

- One crate-wide `Error` enum per crate, in `src/error/mod.rs`, built with
  `thiserror`.
- Fallible public functions return `Result<T>`, the crate alias.
- Add a specific variant instead of stuffing context into a string; error
  messages are lowercase, without trailing punctuation.
- Do not `unwrap()`, `expect()`, or `panic!` in library code paths. They are
  fine in tests, examples, and genuinely unreachable states — where `expect`
  must carry a message explaining the invariant.
- Document a `# Errors` section on every public fallible function and a
  `# Panics` section on anything that can panic.

### Dependencies

Adding a dependency is a design decision. Before adding one, check whether the
standard library or an existing dependency already covers the need. When you do
add one:

- pin a caret range (`serde = "1"`), not an exact version;
- enable only the features you need, with `default-features = false` when that
  meaningfully trims the tree;
- gate anything optional behind a Cargo feature, documented in `Cargo.toml`;
- leave a comment above the entry explaining *why* the crate is needed and what
  uses it — see the existing entries for the expected tone;
- prefer well-maintained crates with a compatible license.

Keep `adapters/lme2-memd/Cargo.lock` committed so CI and candidate builds are
reproducible. Loop scripts take no third-party Python packages.

### Vendored dependencies

Three submodules are pinned by gitlink: `vendor/openhuman`, `vendor/tinyrsi`,
and `benchmarks` (openhuman-benchmarks). During a campaign the vendored
TinyMemory, tinyrsi and tinyhivemind carry local `rsi/*` draft branches
(ADR 0004); their gitlinks here stay at the pin. Initialize them with
`scripts/bootstrap.sh`, which also creates TinyMemory's local `rsi/main` at
the pin when it is missing. OpenHuman's own nested submodules (TinyMemory lives at
`vendor/openhuman/vendor/tinymemory`) are initialized recursively; the
benchmarks' nested OpenHuman copy is not used.

Do not commit edits to vendored code from this repository. Make changes in the
upstream repository, merge them there, then bump the gitlink here in a separate
commit. A vendored change made during a campaign is exported to `patches/`
for a human to upstream.

## Testing

- Loop-script tests live in `loops/lme2-recall/tests/` (`unittest`, standard
  library only) and import the scripts' pure functions; fixtures sit in
  `tests/fixtures/`.
- The memd adapter keeps its unit tests beside the code.
- Pin wire formats in a test: `result.json` is read by tinyrsi, and a field
  name that drifts fails at runtime.
- Use descriptive, behavioral test names: `rejects_an_empty_name`, not
  `test_greet_2`.
- Cover the failure paths, not just the happy path, and assert that no gold
  answer or test-split detail reaches anything the hive sees.
- Tests must be deterministic and independent of network, Docker, wall-clock
  time, and execution order. Gate any live test behind an env var and name it
  `live_*`.

Write the test first when fixing a bug: a failing test that reproduces the
report, then the fix that turns it green.

## Documentation

Write documentation for the reader who has never seen the code.

- Start every script with a docstring saying what it does, how tinyrsi runs
  it, and what it reads and writes.
- Prefer concrete examples over vague description.
- Keep `README.md`, `docs/` and `loops/lme2-recall/README.md` aligned with
  changes in the same commit that changes behavior.
- Write accepted behavior and constraints in `docs/specs/` before creating a
  linked, implementation-ordered plan in `docs/plans/`. Specs define what and
  why; plans define how and in what sequence.
- Keep every Markdown file, including this one, at 500 lines or fewer. When a
  topic outgrows that, split it into focused files and link them from the
  nearest `README.md`.

## Git Workflow

- Never commit directly to `main`. Branch first, one branch per logical change.
- Do feature work in a git worktree so the main checkout stays clean.
- Commit subjects are concise and imperative: `Add retry policy to the client`.
  Keep the subject specific to the change and under ~72 characters.
- Make small, focused commits. Each commit should cover one logical change,
  build independently, and avoid mixing formatting, refactors, and behavior
  changes unless they are inseparable.
- Never commit secrets. `.env` is git-ignored; document new variables in
  `.env.example` with placeholder values.
- Never force-push a shared branch, rewrite published history, or bypass hooks
  with `--no-verify`.

## Pull Requests

Open pull requests ready for review, not as drafts, unless the work genuinely
must not merge yet. A pull request should:

- summarize what changed and why, in a few sentences;
- call out public API or behavior changes explicitly, or state "None";
- list the validation commands actually run, with their outcome;
- link the related issue;
- include updated tests, docs, and examples in the same change.

The template in `.github/PULL_REQUEST_TEMPLATE.md` encodes this checklist.
Address review feedback by fixing it, and reply on each thread describing what
changed. Do not resolve a thread whose feedback you have not addressed or
explicitly declined with a reason.

## Releases

There are no releases. This repository is a research harness: its output is
upstream OpenHuman PRs and committed run reports, not artifacts.

## Agent Working Agreement

For automated contributors specifically:

1. **Read before writing.** Inspect the surrounding module and match its
   conventions, comment density, and idiom rather than importing a house style.
2. **Verify, do not assume.** Run the Build And Test commands and read their
   output before reporting a task complete. Report failures with the output;
   never claim a check passed that you did not run.
3. **Stay in scope.** Implement what was asked. Do not opportunistically
   refactor, reformat, upgrade dependencies, or "fix" unrelated code — raise it
   instead.
4. **No placeholders in delivered code.** No `todo!()`, no stubbed functions, no
   commented-out alternatives left behind. If something cannot be finished, say
   so explicitly.
5. **Do not weaken the guardrails.** Never add blanket `#[allow(...)]`, relax a
   lint, mark a test `#[ignore]`, or loosen CI to get a green run. Fix the
   cause.
6. **Secrets stay out.** Never read, echo, or commit `.env` contents, tokens, or
   credentials, and never paste them into a pull request or issue.
7. **Ask only when blocked.** Make routine judgment calls yourself; escalate
   only irreversible decisions or genuine forks with no clear default.
