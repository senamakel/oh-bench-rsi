#!/usr/bin/env python3
"""Leakage gate: reject a candidate whose diff carries benchmark content.

tinyrsi runs this before any paid evaluation, under its step contract
(``$RSI_CANDIDATE`` and ``$RSI_INCUMBENT`` are TinyMemory worktrees). It reads
the lines the candidate adds relative to the incumbent:

    git -C $RSI_CANDIDATE diff $(git -C $RSI_INCUMBENT rev-parse HEAD)...HEAD

and fails when, case-insensitively, any added line contains

- a benchmark canary (the dataset name, its site names and local hosts);
- a gold answer fragment of six or more characters from the frozen subset,
  unless the incumbent's own source already contains it (a common word such
  as "weight" in the existing code is vocabulary, not new benchmark content);
- any 40-character window of a subset question (windows start every 20
  characters).

A hit exits 1 with a one-line reason on stderr that names the kind of hit but
never the matched text, so the reason tinyrsi records and shows the hive does
not itself leak a gold answer.
"""

import json
import os
import pathlib
import re
import subprocess
import sys

# loops/lme2-recall/gates/leakage.py -> the oh-bench-rsi checkout.
ROOT = pathlib.Path(__file__).resolve().parents[3]
SUBSET = ROOT / "campaigns/lme2/subset.json"
QUESTIONS = ROOT / "benchmarks/.cache/lme-v2-data/questions.jsonl"
CANARIES = ("longmemeval", "lme_v2", "localhost:90", "postmill", "magento", "onestopshop", "one stop market")
ANSWER_CHARS = 6
WINDOW = 40
STRIDE = 20


def added_text(diff: str) -> str:
    """The lower-cased text of every line a unified diff adds."""
    return "\n".join(l[1:] for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++")).lower()


def benchmark_strings(subset: dict, question_lines: list[str]) -> tuple[list[str], list[str]]:
    """Lower-cased subset questions and gold answer fragments."""
    ids = {q["id"] for split in ("dev", "test") for q in subset[split]}
    questions, answers = [], []
    for line in question_lines:
        if not line.strip():
            continue
        row = json.loads(line)
        if row["id"] not in ids:
            continue
        questions.append(row["question"].lower())
        for part in re.split(r"[,;]", str(row["answer"])):
            part = part.strip().lower()
            if len(part) >= ANSWER_CHARS:
                answers.append(part)
    return questions, answers


def leaks(added: str, questions: list[str], answers: list[str], known: str = "") -> dict[str, int]:
    """How many canaries, gold answers and question windows ``added`` contains.

    ``known`` is the lower-cased text the incumbent already holds: a gold
    answer fragment found there is not counted. Canaries and question text
    always are."""
    hits = {
        "canary": sum(c in added for c in CANARIES),
        "gold answer": sum(a in added and a not in known for a in set(answers)),
        "question text": 0,
    }
    for q in questions:
        for start in range(0, max(len(q) - WINDOW, 0) + 1, STRIDE):
            window = q[start:start + WINDOW]
            if len(window) == WINDOW and window in added:
                hits["question text"] += 1
                break
    return {kind: n for kind, n in hits.items() if n}


def git(repo: str, *args: str) -> str:
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout


def known_text(repo: str) -> str:
    """The lower-cased text of every file the incumbent tracks under ``crates/``."""
    parts = []
    for name in git(repo, "ls-files", "-z", "crates").split("\0"):
        path = pathlib.Path(repo) / name
        if name and path.is_file():
            parts.append(path.read_text(encoding="utf-8", errors="replace").lower())
    return "\n".join(parts)


def main() -> int:
    candidate, incumbent = os.environ["RSI_CANDIDATE"], os.environ["RSI_INCUMBENT"]
    if not QUESTIONS.exists():
        print(f"leakage: cannot check, {QUESTIONS.relative_to(ROOT)} is missing (prepare the LongMemEval-V2 data in benchmarks/)", file=sys.stderr)
        return 1
    base = git(incumbent, "rev-parse", "HEAD").strip()
    added = added_text(git(candidate, "diff", f"{base}...HEAD"))
    subset = json.loads(SUBSET.read_text(encoding="utf-8"))
    questions, answers = benchmark_strings(subset, QUESTIONS.read_text(encoding="utf-8").splitlines())
    found = leaks(added, questions, answers, known=known_text(incumbent))
    if found:
        summary = ", ".join(f"{n} {kind}" for kind, n in sorted(found.items()))
        print(f"leakage: benchmark content in the diff ({summary})", file=sys.stderr)
        return 1
    print("leakage: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
