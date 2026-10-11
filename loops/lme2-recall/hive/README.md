# `lme2-recall` hive

The swarm that improves OpenHuman's memory, written in the
[tinyhivemind hive language](../../../vendor/tinyrsi/vendor/tinyhivemind/docs/specs/hive-language.md):
`hive.json` (roles, seats, desk, policies), `constitution.json` (immutable
caps and hard constraints), `memory.json`, `roles/*.md` charters,
`seats/*.md` prompts and shared `context/*.md`.

The `tinyrsi` binary runs it: `tinyrsi run loops/lme2-recall` parses and
validates this package, builds one OpenHuman agent per seat (GLM 5.3 Flash
through the run's spend gateway), adds the hive tools and turn-taking rules,
hands the epoch's brief to the `lead` seat, and runs the desk until it is idle
or `hive.wall_secs` in [`../rsi.json`](../rsi.json) runs out. See
[the loop folder contract](../../../vendor/tinyrsi/docs/specs/rsi-cli.md).

Seat `config.action_dir` values are placeholders tinyrsi resolves:

| Value | Resolved to |
| --- | --- |
| `{incumbent}` | a read-only TinyMemory worktree of `rsi/main` |
| `{candidate}` | the seat's own TinyMemory worktree on `rsi/cand-<run>-e<n>-<seat>` |

## Versions

The roster grows run by run as the swarm proves itself; each growth is a
`roster` edit to this package, recorded in git.

| Version | Seats |
| --- | --- |
| v1 | `lead`, `implementer-a` |
| planned | + `analyst`, `researcher`; then + `implementer-b`, `reviewer` |
