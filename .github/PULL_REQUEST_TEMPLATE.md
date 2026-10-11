## Summary

<!-- What changed and why? Keep it to a few sentences. -->

## Related issue

<!-- Closes #123, or "None". -->

## API or behavior changes

<!-- Note public API or behavior changes, and whether they are breaking. Write "None" if there are none. -->

## Validation

Commands actually run, with their outcome:

- [ ] `python3 -m unittest discover -s loops/lme2-recall/tests`
- [ ] `OPENROUTER_API_KEY=placeholder LADDER_API_KEY=placeholder cargo run --manifest-path vendor/tinyrsi/Cargo.toml -p tinyrsi-cli -- check loops/lme2-recall`
- [ ] `cargo build --manifest-path adapters/lme2-memd/Cargo.toml --target-dir target/memd` (when the adapter changes)
- [ ] No paid run, or its spend and report under `docs/results/`

## Tests

<!-- What tests were added or updated? Note any deliberately untested edge case and why. -->

## Documentation

<!-- Link updated docs, or explain why documentation was not needed. -->

## Checklist

- [ ] The change is focused on one logical change
- [ ] No new `#[allow(...)]`, skipped tests, relaxed lints, or widened loop surface
- [ ] No secrets, tokens, or `.env` contents in the diff or the description
