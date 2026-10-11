#!/usr/bin/env python3
"""One RSI run: observe → swarm → gate → measure → decide → promote, per epoch.

    scripts/bench/with-gateway.sh <run> 5 -- python3 scripts/rsi/epoch.py --run <run> [--epochs 1]

Must run behind the spend gateway (``with-gateway.sh`` exports
``OHRSI_GATEWAY``). Writes the tinyrsi loop trace to ``runs/<run>/events.jsonl``
(what ``apps/rsi-viz`` follows), the swarm's own trace to
``runs/<run>/epoch-<n>/swarm/events.jsonl``, and ``runs/<run>/report.json``.

Nothing leaves the machine except model calls through the gateway: candidate
branches stay local, and promotion fast-forwards the *local* ``rsi/main`` of
the vendored TinyMemory.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import brief  # noqa: E402
import decide  # noqa: E402
import gate  # noqa: E402

TINYMEMORY = ROOT / "vendor/openhuman/vendor/tinymemory"
SWARM_BIN = ROOT / "target/swarm/debug/ohrsi-swarm"
EVAL_COST_FLOOR = 0.6  # a cache-hit candidate eval costs ~$0.2-0.4; keep margin


class LoopTrace:
    def __init__(self, path: pathlib.Path) -> None:
        if path.exists():
            raise SystemExit(f"{path} exists: a run id is never reused")
        path.parent.mkdir(parents=True, exist_ok=True)
        self.file = path.open("a", encoding="utf-8")
        self.seq = 0
        self.t0 = time.monotonic()

    def emit(self, kind: str, **fields) -> None:
        event = {"kind": kind, **fields}
        line = {"seq": self.seq, "elapsed_ms": int((time.monotonic() - self.t0) * 1000), "event": event}
        self.file.write(json.dumps(line, separators=(",", ":")) + "\n")
        self.file.flush()
        self.seq += 1


def gateway_status(gw: str) -> dict:
    with urllib.request.urlopen(f"{gw}/status", timeout=10) as resp:
        return json.loads(resp.read())


def remaining(gw: str) -> float:
    status = gateway_status(gw)
    left = lambda l: l["limit_usd"] - l["settled_usd"] - l["uncertain_usd"] - l["in_flight_usd"]
    return min(left(status["run"]), left(status["total"]))


def head(repo: pathlib.Path) -> str:
    return subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()


def find_summary(tm_head: str) -> pathlib.Path | None:
    for path in sorted(ROOT.glob("runs/*/*/summary.json")) + sorted(ROOT.glob("runs/*/*/*/summary.json")):
        try:
            s = json.loads(path.read_text())
        except ValueError:
            continue
        if s.get("tinymemory_head") == tm_head and not s.get("tinymemory_dirty") and not s.get("limit"):
            return path.parent
    return None


def evaluate(gw: str, run_dir: pathlib.Path, label: str, checkout: pathlib.Path) -> pathlib.Path:
    cmd = [sys.executable, "-I", str(ROOT / "scripts/bench/lme2_eval.py"), "--checkout", str(checkout),
           "--label", label, "--run-dir", str(run_dir), "--gateway", gw]
    subprocess.run(cmd, check=True)
    return run_dir / label


def test_scores(summary_dir: pathlib.Path) -> dict:
    return json.loads((summary_dir / "summary.json").read_text())["splits"]["test"]["scores"]


def test_accuracy(summary_dir: pathlib.Path) -> float:
    return json.loads((summary_dir / "summary.json").read_text())["splits"]["test"]["accuracy"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--run", required=True)
    parser.add_argument("--epochs", type=int, default=1)
    parser.add_argument("--hive", default=str(ROOT / "hives/memory-rsi"))
    parser.add_argument("--swarm-wall-secs", type=int, default=3600)
    args = parser.parse_args()
    gw = os.environ.get("OHRSI_GATEWAY") or sys.exit("run behind scripts/bench/with-gateway.sh")
    run_dir = ROOT / "runs" / args.run
    trace = LoopTrace(run_dir / "events.jsonl")
    report = {"run": args.run, "epochs": [], "started": time.strftime("%Y-%m-%dT%H:%M:%S")}

    incumbent = head(TINYMEMORY)
    trace.emit("run_started", run_id=args.run, subject="OpenHuman TinyMemory recall (LongMemEval-V2 web, small tier)",
               goal={"metric": "lme2_test_accuracy", "direction": "maximize", "target": None})
    inc_dir = find_summary(incumbent)
    if inc_dir is None:
        t = time.monotonic()
        inc_dir = evaluate(gw, run_dir, "incumbent", ROOT / "vendor/openhuman")
        trace.emit("executed", epoch=0, candidate=incumbent[:12], seed=27, metric=test_accuracy(inc_dir), ok=True,
                   duration_ms=int((time.monotonic() - t) * 1000))
    trace.emit("measured", epoch=0, candidate=incumbent[:12], mean=test_accuracy(inc_dir), failures=0, seeds=1)
    history: list[str] = []
    stop = "max_epochs"

    for epoch in range(1, args.epochs + 1):
        trace.emit("epoch_started", epoch=epoch, incumbent=incumbent[:12])
        trace.emit("observed", epoch=epoch, incumbent=incumbent[:12], incumbent_metric=test_accuracy(inc_dir),
                   history=len(history), rejected=sum("rejected" in h for h in history))
        epoch_dir = run_dir / f"epoch-{epoch}"
        epoch_dir.mkdir(parents=True, exist_ok=True)
        brief_path = epoch_dir / "brief.md"
        brief_path.write_text(brief.build(inc_dir, epoch, history), encoding="utf-8")
        if remaining(gw) < EVAL_COST_FLOOR + 0.5:
            stop = "budget"
            break
        swarm = subprocess.run([str(SWARM_BIN), "run", "--hive", args.hive, "--run-dir", str(epoch_dir),
                                "--run-id", f"{args.run}-e{epoch}", "--ledger-dir", str(run_dir), "--brief", str(brief_path),
                                "--gateway", gw, "--tinymemory", str(TINYMEMORY), "--wall-secs", str(args.swarm_wall_secs)],
                               capture_output=True, text=True)
        (epoch_dir / "swarm.log").write_text(swarm.stdout + "\n" + swarm.stderr, encoding="utf-8")
        result_path = epoch_dir / "swarm/result.json"
        epoch_report = {"epoch": epoch, "swarm_exit": swarm.returncode, "candidates": []}
        swarm_result = json.loads(result_path.read_text()) if result_path.exists() else {"candidates": {}}
        epoch_report["swarm"] = {k: swarm_result.get(k) for k in ("reason", "report")}
        for seat, cand in swarm_result["candidates"].items():
            checkout = pathlib.Path(cand["checkout"])
            worktree = checkout / "vendor/tinymemory"
            gated = gate.check(ROOT, worktree, ROOT / "target/candidates")
            cand_head = gated.get("head") or head(worktree)
            entry = {"seat": seat, "branch": cand.get("branch"), "head": cand_head, "gate": gated["checks"],
                     "commits": gate.git(worktree, "log", "--oneline", "rsi/main..HEAD").strip()}
            (epoch_dir / f"gate-{seat}.json").write_text(json.dumps(gated, indent=2), encoding="utf-8")
            trace.emit("ideated", epoch=epoch, proposal_id=f"e{epoch}-{seat}", parent=incumbent[:12],
                       candidate=cand_head[:12] if gated["changed"] else None,
                       rationale=entry["commits"][:300] or "no commits", hypothesis=f"{seat}: {', '.join(gated['changed'][:4])}")
            if not gated["passed"]:
                history.append(f"epoch {epoch} {seat}: rejected at the gate ({gated['checks']})")
                trace.emit("scored", epoch=epoch, candidate=cand_head[:12], decision="reject", estimate=0.0, lower=0.0, upper=0.0)
                epoch_report["candidates"].append(entry)
                continue
            if remaining(gw) < EVAL_COST_FLOOR:
                entry["skipped"] = "budget"
                epoch_report["candidates"].append(entry)
                stop = "budget"
                continue
            t = time.monotonic()
            cand_dir = evaluate(gw, run_dir, f"e{epoch}-{seat}", checkout)
            acc = test_accuracy(cand_dir)
            trace.emit("executed", epoch=epoch, candidate=cand_head[:12], seed=27, metric=acc, ok=True,
                       duration_ms=int((time.monotonic() - t) * 1000))
            trace.emit("measured", epoch=epoch, candidate=cand_head[:12], mean=acc, failures=0, seeds=1)
            verdict = decide.decide(test_scores(inc_dir), test_scores(cand_dir))
            entry["eval"] = {"test_accuracy": acc, "incumbent_test_accuracy": test_accuracy(inc_dir), **verdict}
            trace.emit("scored", epoch=epoch, candidate=cand_head[:12], decision=verdict["decision"],
                       estimate=verdict["estimate"], lower=verdict["lower"], upper=verdict["upper"])
            history.append(f"epoch {epoch} {seat}: {entry['commits'][:120]} → {verdict['decision']} "
                           f"(test {test_accuracy(inc_dir):.3f} → {acc:.3f}, wins {verdict['wins']}, losses {verdict['losses']})")
            if verdict["decision"] == "accept":
                subprocess.run(["git", "-C", str(TINYMEMORY), "merge", "--ff-only", "-q", cand_head], check=True)
                trace.emit("improved", epoch=epoch, **{"from": incumbent[:12]}, to=cand_head[:12], metric=acc)
                incumbent, inc_dir = cand_head, cand_dir
                entry["promoted"] = True
            epoch_report["candidates"].append(entry)
        trace.emit("epoch_finished", epoch=epoch, incumbent=incumbent[:12], incumbent_metric=test_accuracy(inc_dir))
        report["epochs"].append(epoch_report)
        if stop == "budget":
            break

    report["incumbent"] = incumbent
    report["incumbent_test_accuracy"] = test_accuracy(inc_dir)
    report["gateway"] = gateway_status(gw)
    trace.emit("run_finished", reason=stop, incumbent=incumbent[:12], incumbent_metric=test_accuracy(inc_dir))
    (run_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"incumbent": incumbent, "test_accuracy": report["incumbent_test_accuracy"]}))


if __name__ == "__main__":
    main()
