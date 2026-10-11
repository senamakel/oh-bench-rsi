# `ohrsi-swarm` — the memory-rsi hive host

Runs a [tinyhivemind-language](../vendor/tinyrsi/vendor/tinyhivemind/docs/specs/hive-language.md)
package (default [`hives/memory-rsi`](../hives/memory-rsi/README.md)) as a
swarm of OpenHuman agents on a durable tinyhivemind coordinator.

```sh
cargo build --manifest-path swarm/Cargo.toml --target-dir target/swarm
target/swarm/debug/ohrsi-swarm validate hives/memory-rsi
target/swarm/debug/ohrsi-swarm run --run-dir runs/<run>/epoch-1 --brief brief.md \
    --gateway http://127.0.0.1:8790 [--run-id <id>] [--ledger-dir runs/<run>] \
    [--wall-secs 3600] [--turn-secs 1200]
```

Normally `scripts/rsi/epoch.py` invokes it once per epoch.

## What it does

1. **Load** the package: `tinyhivemind_lang::{parse, validate, lower}`.
2. **Build one OpenHuman agent per seat** on one `openhuman-embed` runtime:
   - provider: GLM 5.3 Flash through the spend gateway at
     `/__tag/swarm/<seat>/v1`, so every call is capped and attributed;
   - system prompt: seat prompt + role charter + every `context/*.md`;
   - tools: exactly the seat's `tools.allow` plus the nine permanent
     `hivemind_*` tools, minus `tools.deny`; the `coding` pack is advertised;
   - access and working directory from `config.action_dir`:
     `{candidate}` → `Access::full()` in a fresh TinyMemory worktree on
     `rsi/cand-<run-id>-<seat>` (branched from local `rsi/main`);
     `{incumbent}` → `Access::readonly()` on the vendored TinyMemory.
3. **Coordinate** on `tinyhivemind-hives` with SQLite storage
   (`<run-dir>/swarm/hive.db`) and the package's coordinator settings; the
   host sends the epoch brief privately to the desk's first member (the lead)
   and drains the coordinator until idle or the wall clock.
4. **Trace** to `<run-dir>/swarm/events.jsonl` (kinds documented in
   `src/trace.rs`): every transcript message including seat-to-seat direct
   messages, every tool call (from `TurnHooks::progress`), per-turn usage,
   and the run/total spend read from the gateway ledgers. `apps/rsi-viz`
   renders it as the Swarm view.
5. **Report** `<run-dir>/swarm/result.json`: per editing seat, the branch,
   head, commits and diffstat against `rsi/main`.

Nothing is pushed or merged: candidate branches are local drafts, and
promotion (a local fast-forward of `rsi/main`) is the epoch driver's call
after the gate and the paired evaluation.

## Two pitfalls this host avoids

- `CoordinatorOptions::default()` runs **one** turn at a time. An open
  episode re-nudges its seat every round and sorts first, starving direct
  deliveries to other seats. The host uses the package's own coordinator
  settings (`round_width` 4).
- Seats must **end their turn** after sending; replies arrive as new turns.
  A seat that waits or polls inside its turn holds its slot forever, so
  `wait`/`wait_loop` are denied and the prompt says so.
