"""The epoch brief the lead seat receives.

Built only from the dev split: aggregate accuracy by question type and a
sample of failed dev questions with the model's answer and the recall pack
it was given. Gold answers and every test-split detail stay out.
"""

import json
import pathlib


def build(summary_dir: pathlib.Path, epoch: int, history: list[str], max_cases: int = 10, pack_chars: int = 1500) -> str:
    summary = json.loads((summary_dir / "summary.json").read_text())
    dev = summary["splits"]["dev"]
    lines = [
        f"# Epoch {epoch} brief",
        "",
        f"Incumbent dev accuracy: {dev['accuracy']:.3f} over {dev['n']} questions "
        f"(mean recall pack {dev['pack_tokens_mean']:.0f} tokens).",
        "",
        "## Dev accuracy by question type",
        "",
    ]
    for qtype, row in dev["by_type"].items():
        lines.append(f"- {qtype}: {row['accuracy']:.2f} (n={row['n']})")
    if history:
        lines += ["", "## Earlier epochs", ""] + [f"- {h}" for h in history]
    failures = [json.loads(l) for l in (summary_dir / "dev_failures.jsonl").read_text().splitlines() if l.strip()]
    lines += ["", f"## Sample of failed dev questions ({min(max_cases, len(failures))} of {len(failures)})", ""]
    seen_types = {}
    for row in failures:
        if seen_types.get(row["question_type"], 0) >= 2:
            continue
        seen_types[row["question_type"]] = seen_types.get(row["question_type"], 0) + 1
        pack = row["retrieved_pack"][:pack_chars] or "(empty pack)"
        lines += [
            f"### {row['question_type']}",
            f"Question: {row['question'].strip()[:600]}",
            f"Model answer: {row['model_answer'] or '(none)'}{' [answered unknown]' if row['answered_unknown'] else ''}",
            "Recall pack the model saw (truncated):",
            "```",
            pack,
            "```",
            "",
        ]
        if sum(seen_types.values()) >= max_cases:
            break
    lines += [
        "## Your task",
        "",
        "Find a general weakness in how TinyMemory gathers, ranks, excerpts or renders recalled",
        "evidence into the pack that explains failures like these, and have the implementer fix",
        "it with a unit test. Do not tune for these specific questions; the change is judged on",
        "held-out questions and must make sense for any OpenHuman user with long agent histories.",
    ]
    return "\n".join(lines) + "\n"
