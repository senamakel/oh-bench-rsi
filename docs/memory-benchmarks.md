# Memory benchmarks

What "ace the memory benchmarks" means today, where OpenHuman stands, and which
failures the loop targets. All benchmark code lives in the frozen
`benchmarks/` submodule; this page only summarizes it.

## Suites

| Suite | Path in `benchmarks/` | What it measures | State |
|---|---|---|---|
| Memory audit (issue #27) | `memory/` | 20 pinned questions: 7 LongMemEval-S (one per category incl. abstention) + 13 LoCoMo (dialogue 0). Four arms: no-memory, full-context, reference-memory, `oh-tinymemory-cortex-direct`. Official LME judge prompt and LoCoMo category F1. | Runnable, paid (OpenRouter), ~$2 per full run |
| LongMemEval-V2 | `longmemeval-v2/` | 451 questions, web + enterprise, multimodal. Official `Memory.insert` / `Memory.query` contract. | **The RSI target.** OH adapter in `adapters/` (this repo), measured by the [`lme2-recall` loop](../loops/lme2-recall/README.md); see [the loop spec](specs/rsi-memory-loop.md) for the configuration. Baseline (`run-02`): dev 0.320, test 0.160 |
| Compaction probes | `compaction/` | Truth probes over compacted context. | Offline tooling |

The memory arm calls TinyMemory's `CortexEngine` and `AgentMemory::pre_turn`
directly. It does not exercise the desktop agent or core JSON-RPC path.

## Baseline (run `memory27-glm53flash`)

GLM 5.3 Flash, low reasoning; judge `openai/gpt-4o-mini-2024-07-18`. Questions
scored by every arm:

| Dataset | no-memory | full-context | reference-memory | **OH memory** |
|---|---:|---:|---:|---:|
| LongMemEval (6) | 16.7% | 66.7% | 33.3% | **50.0%** |
| LoCoMo (13) | 18.5% | 28.5% | 27.1% | **22.2%** |

Full context is the practical ceiling for this subset: the goal is for the OH
memory arm to reach and then beat it at a fraction of the prompt tokens. This
is a 20-question smoke subset; no population claim follows from it.

Source: `benchmarks/results/memory27-glm53flash/audit.md`,
`benchmarks/memory/FINDINGS.md`.

## Known failures

The `lme2-recall` loop's editable surface (`tinymemory-tools`,
`tinymemory-integrations`) covers the recall-side findings; the others are
handled outside the loop.

| Finding (`memory/FINDINGS.md`) | Code | Handling |
|---|---|---|
| #2 Retrieved answer dropped during pack trimming | TinyMemory `tinymemory-tools/src/recall/{gather,render}.rs` | `lme2-recall` loop |
| #4 Temporal answers lose event ordering | OH `memory/lifecycle/date_hint.rs`, TinyMemory recall ranking | `lme2-recall` loop (TinyMemory side only) |
| #3 "Team conversations" includes the current agent | TinyMemory `tinymemory-tools/src/lifecycle/mod.rs` | Direct bug fix, not RSI (deterministic repro: `node memory/repro-team.mjs`) |
| #1 Recall deadline exceeded on slow engines | OH `config/schema/memory.rs`, `memory/lifecycle/hooks.rs` | Default since raised to 10 s at the current pin; needs a re-measure, not a campaign |
| #5 Low LoCoMo F1 partly answer formatting | Answer prompt | Out of scope until the measurement is agreed |

## Caveats to carry into every run

- Enrichment readiness is approximated by three quiet polls; record it.
- Each native task needs fresh CortexDB storage; never share a server across tasks.
- The memory audit ledgers spend in `results/<run>/budget.json`; the loop's
  tinyrsi gateway ledgers it in `~/.tinyrsi/spend.json`. Read the relevant
  one before a paid run.
