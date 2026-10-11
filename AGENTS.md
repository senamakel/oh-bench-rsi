# Repository Guidelines

This file is the single source of truth for how humans and coding agents work
in this repository. `CLAUDE.md` is a symlink to this file, so every agent reads
the same instructions.

Keep it current: when the layout, commands, or rules change, update this file
in the same commit.

## Mission

Make OpenHuman ace its memory benchmarks through recursive self-improvement:
an automated loop proposes changes to OpenHuman's memory code, measures them on
frozen benchmarks, and keeps only verified improvements. Read
[`docs/memory-benchmarks.md`](docs/memory-benchmarks.md) and
[`docs/specs/rsi-memory-loop.md`](docs/specs/rsi-memory-loop.md) before working
on the loop or a campaign.

## RSI Rules

These are non-negotiable for humans, agents, and the loop's own proposer.

1. **The measurement is frozen.** Never edit `benchmarks/` — datasets, task
   selection, prompts, judges, scorers, budgets, or the gateway — to raise a
   score. Benchmark changes go upstream to openhuman-benchmarks as their own,
   separately reviewed PR, and reset every baseline.
2. **Edit only the declared surface.** A campaign's `surface.editable` paths,
   all under `vendor/openhuman/`, are the only files a proposal may change.
   `ohrsi check` enforces this; do not weaken the validator.
3. **No leakage.** Gold answers, judge outputs, and private question metadata
   never reach the proposer or the system under test. Do not hard-code
   benchmark questions, entities, or answers into OpenHuman.
4. **Paired, repeated evidence.** Compare candidate and incumbent on the same
   seeds. One lucky run is not an improvement; report inconclusive results as
   inconclusive.
5. **Spend is capped.** Respect each campaign's `budget.usd_cap` and read the
   live budget ledger before a paid run. Never loop on paid calls unattended
   without a cap.
6. **Stay local.** Candidates and promotions live on local `rsi/*` branches
   in the vendored repositories (ADR 0004). The harness never pushes, merges,
   or opens pull requests; a human upstreams promoted diffs, each with its
   regression test, from the run reports. Never commit vendored code into
   this repository's history.
7. **Keep OpenHuman green.** A candidate that fails OpenHuman's or
   TinyMemory's own tests is rejected regardless of its score.

## Project Structure

```text
Cargo.toml              # virtual workspace (crates/* only; CI builds it without submodules)
clippy.toml             # product names clippy's doc_markdown accepts
crates/
├── ohrsi/              # campaign manifests: schema, validation, `ohrsi check|list`
└── ohrsi-gateway/      # spend-capped OpenRouter proxy: the only path to a model
swarm/                  # standalone crate: runs a hive package as OpenHuman agents
hives/memory-rsi/       # the swarm, in the tinyhivemind hive language
adapters/
├── lme2-memd/          # standalone crate: OpenHuman TinyMemory over loopback HTTP,
│                       # built per candidate against its TinyMemory checkout
└── lme2/               # the official harness's `Memory` backend + runner
scripts/
├── bench/              # with-gateway.sh, lme2_eval.py, lme2_subset.py
└── rsi/                # epoch.py (one run), gate.py, decide.py, brief.py
campaigns/              # campaign manifests; lme2/subset.json is the frozen subset
runs/                   # git-ignored: ledgers, meters, traces, candidates, ingest cache
vendor/openhuman/       # pinned OpenHuman (TinyMemory at vendor/tinymemory)
vendor/tinyrsi/         # pinned tinyrsi (tinyhivemind nested; rsi-viz in apps/)
benchmarks/             # pinned openhuman-benchmarks — frozen measurement
docs/                   # memory-benchmarks.md, specs/, plans/, adr/, results/
```

### Running

```sh
scripts/bootstrap.sh                                  # submodules, checks
cargo build --manifest-path swarm/Cargo.toml --target-dir target/swarm
scripts/bench/with-gateway.sh <run> 5 -- python3 scripts/rsi/epoch.py --run <run>
cd vendor/tinyrsi/apps/rsi-viz && RSI_RUN_ROOT=../../../../runs npm run dev
```

The LongMemEval-V2 harness's lockfile is Linux-only; it runs in the
`ghcr.io/astral-sh/uv:python3.11-bookworm` container, which reaches the host's
loopback gateway and memd as `host.docker.internal`.

Each feature area belongs in a focused module directory under a crate's `src/`.
A module root explains the module, wires its pieces together, and exposes the
smallest useful API. Move substantial type definitions into `types.rs` and put
module-local unit tests in a dedicated `test.rs`, wired from the bottom of the
module root with:

```rust
#[cfg(test)]
mod test;
```

Do not accumulate inline `mod tests` blocks in implementation files, and do not
let a general-purpose `utils.rs` or `helpers.rs` grow. Add a crate by creating
`crates/<name>/`; inherit `[workspace.package]` fields and opt into
`[lints] workspace = true`.

Keep public exports centralized in each crate's `src/lib.rs`. Put shared error
variants in `src/error/mod.rs` and return the crate-wide `Result<T>`.

## Build And Test

Run every command from the repository root. These four are the contract; CI
runs exactly them, so a green local run should mean a green CI run.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

Supporting commands:

- `cargo fmt --all` — format before committing.
- `cargo test <filter>` — run a focused subset while iterating.
- `cargo test -p ohrsi` — run one crate's suite.
- `cargo run -p ohrsi -- check` — validate every campaign manifest.
- `scripts/bootstrap.sh` — initialize the submodules and run `ohrsi check`.
- `cargo doc --no-deps --all-features` — build the rustdoc CI also builds with
  `RUSTDOCFLAGS="-D warnings"`.
- `cargo test --doc` — run doctests alone when editing documentation examples.

Never skip, ignore, or delete a failing test to make a command pass. Fix the
root cause, or stop and report the blocker.

## Coding Style

Use standard `rustfmt` output and Rust 2024 idioms. Do not hand-format around
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
- `unsafe` is forbidden workspace-wide by `[workspace.lints]` in the root
  `Cargo.toml`. If a project genuinely needs it, relax the lint in its own
  commit and document every invariant with a `// SAFETY:` comment.

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
- declare it once in the root `[workspace.dependencies]` when more than one
  crate needs it, and take it with `{ workspace = true }`;
- leave a comment above the entry explaining *why* the crate is needed and what
  uses it — see the existing entries for the expected tone;
- prefer well-maintained crates with a compatible license.

Keep `Cargo.lock` committed; this workspace ships a single lockfile so CI and
releases are reproducible.

### Vendored dependencies

Three submodules are pinned by gitlink: `vendor/openhuman`, `vendor/tinyrsi`,
and `benchmarks` (openhuman-benchmarks). During a campaign the vendored
TinyMemory, tinyrsi and tinyhivemind carry local `rsi/*` draft branches
(ADR 0004); their gitlinks here stay at the pin. Initialize them with
`scripts/bootstrap.sh`. OpenHuman's own nested submodules (TinyMemory lives at
`vendor/openhuman/vendor/tinymemory`) are initialized recursively; the
benchmarks' nested OpenHuman copy is not used.

Do not commit edits to vendored code from this repository. Make changes in the
upstream repository, merge them there, then bump the gitlink here in a separate
commit. The `ohrsi` crate must keep building without any submodule checked out,
so CI stays cheap.

## Testing

- Module-local unit tests live in `crates/<crate>/src/<feature>/test.rs` and may
  touch private items.
- Integration tests live in `crates/<crate>/tests/` and exercise only the public
  API — they are the regression suite for the crate's contract.
- Payload types pin their serde representation in a unit test. That
  representation is the wire form: a host and a module that disagree about a
  field name fail at runtime with a decode error.
- Use descriptive, behavioral test names: `rejects_an_empty_name`, not
  `test_greet_2`.
- Cover the failure paths, not just the happy path. Every new error variant
  needs a test that produces it.
- For async behavior, standardize on one runtime (`tokio` as a dev-dependency
  for tests) rather than mixing runtimes.
- Tests must be deterministic and independent of network, wall-clock time, and
  execution order. Gate any live/network test behind a feature or an env var and
  name it `live_*` so it is easy to exclude.
- Maintain at least 90% line coverage in every source file. Add or update tests
  with every behavior change, and note any deliberately untested edge case in
  the pull request description.

Write the test first when fixing a bug: a failing test that reproduces the
report, then the fix that turns it green.

## Documentation

Write documentation for the reader who has never seen the code.

- Every public item gets a rustdoc comment. `missing_docs` is a warning that CI
  treats as an error.
- Start every `mod.rs` and `test.rs` with a concise module-level `//!`
  description.
- Each crate's `src/lib.rs` carries its crate-level overview: what the crate
  does, the primary entry points, and a short runnable example. It should also
  say what the crate deliberately does *not* hold, and why.
- Prefer concrete examples over vague description. Doc examples are compiled and
  run by `cargo test`, so they cannot drift.
- Complex modules must include a module-level `README.md` covering their design,
  public surface, and important operational constraints.
- Keep `README.md`, `docs/`, and module docs aligned with code changes in the
  same commit that changes behavior.
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
upstream OpenHuman PRs and committed run reports, not artifacts. Do not
hand-edit the `version` in `[workspace.package]`.

## Agent Working Agreement

For automated contributors specifically:

1. **Read before writing.** Inspect the surrounding module and match its
   conventions, comment density, and idiom rather than importing a house style.
2. **Verify, do not assume.** Run the four contract commands and read their
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
