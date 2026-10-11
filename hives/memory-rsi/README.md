# `memory-rsi` hive

The swarm that improves OpenHuman's memory, written in the
[tinyhivemind hive language](../../vendor/tinyrsi/vendor/tinyhivemind/docs/specs/hive-language.md):
`hive.json` (roles, seats, desk, policies), `constitution.json` (immutable
caps and hard constraints), `memory.json`, `roles/*.md` charters,
`seats/*.md` prompts and shared `context/*.md`.

`crates`-independent host code in [`swarm/`](../../swarm/README.md) parses,
validates and lowers this package, builds one OpenHuman agent per seat
(GLM 5.3 Flash through the spend gateway), and runs the desk on a durable
tinyhivemind coordinator.

Seat `config.action_dir` values are host placeholders:

| Value | Resolved by the host to |
| --- | --- |
| `{incumbent}` | a read-only TinyMemory checkout at `rsi/main` |
| `{candidate}` | the seat's own TinyMemory worktree on `rsi/cand-<run>-<seat>` |

## Versions

The roster grows run by run as the swarm proves itself; each growth is a
`roster` edit to this package, recorded in git.

| Version | Seats |
| --- | --- |
| v1 | `lead`, `implementer-a` |
| planned | + `analyst`, `researcher`; then + `implementer-b`, `reviewer` |
