# Campaign manifests

- **Status:** Superseded by the tinyrsi loop folder (ADR 0007)

A campaign used to be a TOML file in `campaigns/` validated by the `ohrsi`
crate (`ohrsi check`). Both are gone. The loop is now declared by
[`loops/lme2-recall/rsi.json`](../../loops/lme2-recall/rsi.json) and validated
by `tinyrsi check loops/lme2-recall`, under tinyrsi's
[loop folder contract](../../vendor/tinyrsi/docs/specs/rsi-cli.md).

| Campaign field | Loop folder equivalent |
| --- | --- |
| `name`, `brief` | `name`, `brief` (`brief.md`, a template) |
| `[benchmark] command`, `timeout_secs` | `score.scorers[].run`, `timeout_secs` |
| `[metric]` | `score.decide` (`on`, `test`, `split`, `alpha`) |
| `[surface] editable`, `frozen` | `subject.editable`, `subject.frozen` (relative to the TinyMemory checkout) |
| `[evaluation] seeds` | `score.decide.split` — the frozen dev/test split replaces seeds |
| `[budget] usd_cap`, `max_epochs` | `budget.run_usd`, `budget.total_usd`, `epochs`, `patience` |

`campaigns/lme2/subset.json`, the frozen question subset, remains and is read
by the scorer and the leakage gate.
