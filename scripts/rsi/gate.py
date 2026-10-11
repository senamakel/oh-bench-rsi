"""The candidate gate: everything checked *before* any paid evaluation.

A candidate is a TinyMemory worktree on ``rsi/cand-*``. It passes when:

1. **surface** — every changed path is under an editable prefix and none is
   under a frozen one (the engine contract ``tinymemory-api`` is frozen);
2. **leakage** — no added line contains benchmark content: any subset
   question text (40+ character windows), any gold answer phrase of 6+
   characters, or a benchmark canary (site names, the dataset name);
3. **tests** — the diff adds or changes a test file;
4. **green** — ``cargo test`` passes for every touched crate.

Uncommitted work is committed first (``swarm: uncommitted changes``) so the
gate and the evaluation see the same tree.
"""

import json
import pathlib
import re
import subprocess

EDITABLE = ("crates/tinymemory-tools/", "crates/tinymemory-integrations/")
FROZEN = ("crates/tinymemory-api/",)
CANARIES = ("longmemeval", "lme_v2", "localhost:90", "postmill", "magento", "onestopshop", "one stop market")
TEST_FILE = re.compile(r"(^|/)(test\.rs|[^/]*_tests?\.rs|tests/[^/]+\.rs)$")


def git(repo: pathlib.Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout


def commit_pending(worktree: pathlib.Path) -> bool:
    if not git(worktree, "status", "--porcelain").strip():
        return False
    git(worktree, "add", "-A")
    git(worktree, "commit", "-q", "-m", "swarm: uncommitted changes")
    return True


def benchmark_strings(root: pathlib.Path) -> tuple[list[str], list[str]]:
    subset = json.loads((root / "campaigns/lme2/subset.json").read_text())
    ids = {q["id"] for split in ("dev", "test") for q in subset[split]}
    questions, answers = [], []
    for line in (root / "benchmarks/.cache/lme-v2-data/questions.jsonl").read_text().splitlines():
        if line.strip():
            row = json.loads(line)
            if row["id"] in ids:
                questions.append(row["question"].lower())
                for part in re.split(r"[,;]", str(row["answer"])):
                    part = part.strip().lower()
                    if len(part) >= 6:
                        answers.append(part)
    return questions, answers


def check(root: pathlib.Path, worktree: pathlib.Path, target_dir: pathlib.Path) -> dict:
    autocommitted = commit_pending(worktree)
    changed = [p for p in git(worktree, "diff", "--name-only", "rsi/main...HEAD").splitlines() if p]
    result = {"changed": changed, "autocommitted": autocommitted, "checks": {}}
    if not changed:
        result["checks"]["surface"] = "no changes"
        result["passed"] = False
        return result
    bad = [p for p in changed if not p.startswith(EDITABLE) or p.startswith(FROZEN)]
    result["checks"]["surface"] = "ok" if not bad else f"outside the editable surface: {bad}"
    added = "\n".join(l[1:] for l in git(worktree, "diff", "rsi/main...HEAD").splitlines()
                      if l.startswith("+") and not l.startswith("+++")).lower()
    questions, answers = benchmark_strings(root)
    hits = [c for c in CANARIES if c in added]
    hits += [a for a in answers if a in added]
    for q in questions:
        for start in range(0, max(len(q) - 40, 0) + 1, 20):
            window = q[start:start + 40]
            if len(window) == 40 and window in added:
                hits.append(window)
                break
    result["checks"]["leakage"] = "ok" if not hits else f"benchmark content in the diff: {sorted(set(hits))[:5]}"
    result["checks"]["tests"] = "ok" if any(TEST_FILE.search(p) for p in changed) else "no test file changed"
    crates = sorted({p.split("/")[1] for p in changed if p.startswith("crates/")})
    log = []
    green = True
    for crate in crates:
        proc = subprocess.run(["cargo", "test", "-q", "-p", crate, "--target-dir", str(target_dir)],
                              cwd=worktree, capture_output=True, text=True)
        log.append(f"$ cargo test -p {crate}\n{proc.stdout[-3000:]}\n{proc.stderr[-3000:]}")
        green &= proc.returncode == 0
    result["checks"]["green"] = "ok" if green else "cargo test failed"
    result["test_log"] = "\n".join(log)
    result["passed"] = all(v == "ok" for v in result["checks"].values())
    result["head"] = git(worktree, "rev-parse", "HEAD").strip()
    return result
