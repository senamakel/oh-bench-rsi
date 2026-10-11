# Campaign manifests

- **Status:** Implemented (validation); execution pending
- **Code:** `crates/ohrsi/src/campaign/`

A campaign is one TOML file in `campaigns/`. `ohrsi check` validates every one.

```toml
name = "memory-recall-pack"          # lowercase kebab-case
brief = "Shown verbatim to the proposer: the goal and the rules."

[benchmark]
workdir = "."                         # repo-relative; "." is the root
command = ["scripts/bench/memory-metric.sh", "--arm", "oh-tinymemory-cortex-direct"]
env = ["OPENROUTER_API_KEY"]          # passed through; all else cleared
timeout_secs = 7200                   # per seed

[metric]
name = "memory_accuracy"              # from `METRIC <name>=<value>`
direction = "maximize"                # or "minimize"
target = 0.7                          # optional early stop

[surface]
editable = ["vendor/openhuman/..."]  # files/dirs a proposal may change
frozen = ["vendor/openhuman/vendor/tinymemory/crates/tinymemory-api"]

[evaluation]
seeds = [27, 28, 29]                  # ≥ 2, distinct

[budget]
usd_cap = 10.0                        # > 0, finite
max_epochs = 4                        # ≥ 1
proposals_per_epoch = 2               # ≥ 1
```

## Rules

| Rule | Error |
|---|---|
| Unknown keys | `Parse` |
| Name not kebab-case | `InvalidName` |
| Empty command | `EmptyCommand` |
| Absolute, `..`, `.`, or backslash path | `UnsafePath` |
| Empty metric name / non-finite target | `EmptyMetricName` / `NonFiniteTarget` |
| No editable paths | `NoEditable` |
| Editable path not strictly under `vendor/openhuman/` | `OutsideOpenHuman` |
| Editable path inside or enclosing a frozen path (`benchmarks` always frozen) | `FrozenEditable` |
| < 2 seeds / repeated seed | `TooFewSeeds` / `DuplicateSeed` |
| Bad cap / zero limit | `InvalidSpendCap` / `ZeroLimit` |

When `vendor/openhuman` is checked out, `ohrsi check` also fails if a surface
path does not exist (upstream moved the file; update the campaign).
