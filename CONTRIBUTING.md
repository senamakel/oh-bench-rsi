# Contributing

Thanks for contributing. The best changes here are small, explicit, tested, and
easy to review. [`AGENTS.md`](AGENTS.md) holds the full repository guidelines —
this document is the short path through them.

## Development Setup

Install Python 3.11+ and a stable Rust toolchain (for the `tinyrsi` binary and
the memd adapter), initialize the submodules, then run the checks CI runs:

```sh
scripts/bootstrap.sh
python3 -m unittest discover -s loops/lme2-recall/tests
OPENROUTER_API_KEY=placeholder cargo run --manifest-path vendor/tinyrsi/Cargo.toml \
  -p tinyrsi-cli -- check loops/lme2-recall
cargo build --manifest-path adapters/lme2-memd/Cargo.toml --target-dir target/memd
```

None of these spends money. A paid run (`tinyrsi run loops/lme2-recall`) is a
deliberate act with its report committed under `docs/results/`.

## Making A Change

1. Branch from `main` — never commit directly to it. If you use the `worktree`
   helper, work inside `worktrees/<slug>`.
2. Loop changes go in `loops/lme2-recall/`. Keep step scripts standard-library
   Python with their pure parts unit-tested in `loops/lme2-recall/tests/`.
3. Never change the measurement (`benchmarks/`, the scorer's models, image,
   ingest environment or subset) to raise a score.
4. Add or update tests with every behavior change, covering the failure paths.
5. Changes to the generic loop machinery belong upstream in tinyrsi; bump
   `vendor/tinyrsi` in its own commit once merged there.
6. Update `README.md` and `docs/` in the same commit when behavior or usage
   changes.

Do not skip tests, relax lints, or widen the loop's editable surface to get a
green run. Fix the cause, or raise the blocker in the pull request.

## Adding A Dependency

Loop scripts take no third-party Python packages. For the memd adapter, check
whether the standard library or an existing dependency already covers the
need; if not, add it with a caret range, only the features you need, and a
comment saying why it exists.

## Pull Request Checklist

- [ ] the loop unit tests pass
- [ ] `tinyrsi check loops/lme2-recall` prints `ok: lme2-recall ...`
- [ ] the memd adapter builds, when it changed
- [ ] tests added or updated for behavior changes
- [ ] documentation updated for behavior or usage changes
- [ ] the pull request is focused on one logical change

Open pull requests ready for review rather than as drafts, unless the work
genuinely must not merge yet — say in the body what has to happen first.

## Commit Style

Use concise, imperative subjects scoped to one logical change:

```text
Add retry policy to the client
Document the error taxonomy
```

Avoid mixing formatting, refactors, and behavior changes unless they are
inseparable. Never commit secrets; `.env` is git-ignored and new variables are
documented in `.env.example` with placeholder values.

## Issue Triage

Good issues include the version or commit, the toolchain, the relevant module or
API, a minimal reproduction, the expected and actual behavior, and the commands
run. Feature requests should explain the workflow they unlock and the public API
shape they imply.

## Security

Do not report vulnerabilities through public issues. Follow the process in
[SECURITY.md](SECURITY.md).
