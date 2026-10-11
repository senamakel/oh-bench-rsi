#!/usr/bin/env python3
"""Freeze the LongMemEval-V2 question subset the RSI loop measures.

Selects text-only questions of one domain on the small tier (which shares
one 100-trajectory haystack per domain), and splits them, stratified by
question type with a fixed seed, into:

- ``dev``  — the swarm may see per-question failures here;
- ``test`` — only the aggregate score is ever reported; promotion is
  decided on this split.

Writes ``campaigns/lme2/subset.json`` with each question's id, type and
the SHA-256 of its exact ``questions.jsonl`` line, so a changed dataset is
detected rather than silently measured. Question text and answers are not
written.
"""

import argparse
import collections
import hashlib
import json
import pathlib
import random

ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA = ROOT / "benchmarks/.cache/lme-v2-data"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--domain", default="web")
    parser.add_argument("--seed", type=int, default=27)
    parser.add_argument("--dev-fraction", type=float, default=1 / 3)
    parser.add_argument("--out", default=str(ROOT / "campaigns/lme2/subset.json"))
    args = parser.parse_args()

    rows = []
    for line in (DATA / "questions.jsonl").read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row["domain"] == args.domain and not row.get("image"):
            rows.append((row, hashlib.sha256(line.encode("utf-8")).hexdigest()))

    by_type = collections.defaultdict(list)
    for row, digest in rows:
        by_type[row["question_type"]].append({"id": row["id"], "type": row["question_type"], "sha256": digest})
    rng = random.Random(args.seed)
    dev, test = [], []
    for qtype in sorted(by_type):
        group = sorted(by_type[qtype], key=lambda q: q["id"])
        rng.shuffle(group)
        cut = round(len(group) * args.dev_fraction)
        dev.extend(group[:cut])
        test.extend(group[cut:])

    haystacks = json.loads((DATA / "haystacks/lme_v2_small.json").read_text(encoding="utf-8"))
    haystack = haystacks[rows[0][0]["id"]]
    assert all(haystacks[row["id"]] == haystack for row, _ in rows), "small tier shares one haystack per domain"
    out = {
        "dataset": "xiaowu0162/longmemeval-v2",
        "tier": "small",
        "domain": args.domain,
        "seed": args.seed,
        "selection": "text-only questions (no question image), stratified by question_type",
        "haystack_sha256": hashlib.sha256(json.dumps(haystack).encode("utf-8")).hexdigest(),
        "haystack_size": len(haystack),
        "dev": dev,
        "test": test,
    }
    path = pathlib.Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {path}: dev={len(dev)} test={len(test)}")


if __name__ == "__main__":
    main()
