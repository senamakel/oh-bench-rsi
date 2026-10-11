# Memory benchmarks

What "ace the memory benchmarks" means today, where OpenHuman stands, and which
failures the first campaigns target. All benchmark code lives in the frozen
`benchmarks/` submodule; this page only summarizes it.

## Suites

| Suite | Path in `benchmarks/` | What it measures | State |
|---|---|---|---|
| Memory audit (issue #27) | `memory/` | 20 pinned questions: 7 LongMemEval-S (one per category incl. abstention) + 13 LoCoMo (dialogue 0). Four arms: no-memory, full-context, reference-memory, `oh-tinymemory-cortex-direct`. Official LME judge prompt and LoCoMo category F1. | Runnable, paid (OpenRouter), ~$2 per full run |
| LongMemEval-V2 | `longmemeval-v2/` | 451 questions, web + enterprise, multimodal. Official `Memory.insert` / `Memory.query` contract. | Data and harness set up; **no OH adapter yet** |
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

## Known failures → campaigns

| Finding (`memory/FINDINGS.md`) | Code | Campaign |
|---|---|---|
| #2 Retrieved answer dropped during pack trimming | TinyMemory `tinymemory-tools/src/recall/{gather,render}.rs` | `memory-recall-pack` |
| #4 Temporal answers lose event ordering | OH `memory/lifecycle/date_hint.rs`, TinyMemory recall ranking | `memory-temporal-recall` |
| #3 "Team conversations" includes the current agent | TinyMemory `tinymemory-tools/src/lifecycle/mod.rs` | Direct bug fix, not RSI (deterministic repro: `node memory/repro-team.mjs`) |
| #1 Recall deadline exceeded on slow engines | OH `config/schema/memory.rs`, `memory/lifecycle/hooks.rs` | Default since raised to 10 s at the current pin; needs a re-measure, not a campaign |
| #5 Low LoCoMo F1 partly answer formatting | Answer prompt | Out of scope until the measurement is agreed |

## Caveats to carry into every run

- Enrichment readiness is approximated by three quiet polls; record it.
- Each native task needs fresh CortexDB storage; never share a server across tasks.
- Spend is ledgered in `results/<run>/budget.json`; read it before a paid run.
