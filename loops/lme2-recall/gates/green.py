#!/usr/bin/env python3
"""Green gate: TinyMemory's own tests pass for every crate the candidate touches.

tinyrsi runs this after the leakage gate, under its step contract. It lists
the files the candidate changed relative to the incumbent, and runs

    cargo test -q -p <crate> --target-dir <oh-bench-rsi>/target/candidates

in the candidate worktree for each ``crates/<crate>/`` among them. One shared
target directory keeps candidate builds warm. A failure exits 1 with the
failing crate as the reason; the full output is kept in ``$RSI_OUT``.
"""

import os
import pathlib
import subprocess
import sys

# loops/lme2-recall/gates/green.py -> the oh-bench-rsi checkout.
ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = ROOT / "target/candidates"
LOG_TAIL = 3000


def touched_crates(paths: list[str]) -> list[str]:
    """Sorted crate directory names under ``crates/`` that ``paths`` touch."""
    return sorted({p.split("/")[1] for p in paths if p.startswith("crates/") and p.count("/") >= 2})


def git(repo: str, *args: str) -> str:
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True, text=True).stdout


def main() -> int:
    candidate, incumbent = os.environ["RSI_CANDIDATE"], os.environ["RSI_INCUMBENT"]
    out = pathlib.Path(os.environ.get("RSI_OUT", "."))
    base = git(incumbent, "rev-parse", "HEAD").strip()
    changed = [p for p in git(candidate, "diff", "--name-only", f"{base}...HEAD").splitlines() if p]
    failed = []
    with (out / "cargo-test.log").open("w", encoding="utf-8") as log:
        for crate in touched_crates(changed):
            proc = subprocess.run(["cargo", "test", "-q", "-p", crate, "--target-dir", str(TARGET)],
                                  cwd=candidate, capture_output=True, text=True)
            log.write(f"$ cargo test -q -p {crate}\n{proc.stdout[-LOG_TAIL:]}\n{proc.stderr[-LOG_TAIL:]}\n")
            print(f"cargo test -p {crate}: {'ok' if proc.returncode == 0 else 'FAILED'}")
            if proc.returncode != 0:
                failed.append(crate)
    if failed:
        print(f"green: cargo test failed for {', '.join(failed)}", file=sys.stderr)
        return 1
    print("green: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
