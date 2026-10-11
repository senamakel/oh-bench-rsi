#!/usr/bin/env python3
"""Measure one OpenHuman checkout on the frozen LongMemEval-V2 subset.

    lme2_eval.py --checkout <openhuman dir> --label <name> --run-dir runs/<run>
                 --gateway http://127.0.0.1:<port> [--engine cortex|reference]
                 [--limit N]

Steps, each logged to ``<run-dir>/<label>/eval.log``:

1. Build ``lme2-memd`` against the checkout's own TinyMemory crates.
2. Start a fresh CortexDB container whose LLM and embedding traffic goes
   through the spend gateway (``/__tag/ingest/cortex``). Its data directory
   is restored from the ingest cache when the haystack *and* the write path
   (TinyMemory's api/integrations sources, the adapter's converter, the
   CortexDB image and config) are unchanged; otherwise it ingests and the
   result is cached for later candidates.
3. Run the unmodified official harness in its Linux container with the
   ``oh_tinymemory`` backend, GLM 5.3 Flash as reader and judge, both
   through the gateway (``/__tag/reader``, ``/__tag/judge``).
4. Score per split and write ``summary.json``, ``dev_failures.jsonl`` (dev
   only: question, the model's answer, verdict, retrieved pack — never the
   gold answer) and print ``METRIC lme2_test_accuracy=<x>`` lines.

Everything here runs locally; the only network egress is the gateway's.
"""

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
BENCH = ROOT / "benchmarks"
DATA = BENCH / ".cache/lme-v2-data"
OFFICIAL = BENCH / ".cache/longmemeval-v2"
VENV_PY = BENCH / ".cache/lme-v2-venv/bin/python"
MEMD_SRC = ROOT / "adapters/lme2-memd"
CORTEX_IMAGE = "cortexdb/cortexdb@sha256:636e0ed0b8cd4ae2f307e3a90f99050f7879f73c556b1ccbbabf2d3a383398fc"
CORTEX_TOML = BENCH / "memory/cortex.toml"
HARNESS_IMAGE = "ghcr.io/astral-sh/uv:python3.11-bookworm"
MODEL = "z-ai/glm-5.3-flash"
EMBED = "openai/text-embedding-3-small"
# The whole oh-bench-rsi checkout is mounted at its own path in containers.
MOUNT_ROOT = pathlib.Path(os.environ.get("OHRSI_MOUNT_ROOT", str(ROOT.parents[1] if ROOT.parent.name == "worktrees" else ROOT)))
WRITE_PATH = [
    "vendor/tinymemory/crates/tinymemory-api",
    "vendor/tinymemory/crates/tinymemory-integrations",
]


class Log:
    def __init__(self, path: pathlib.Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        self.file = path.open("a", encoding="utf-8")

    def __call__(self, message: str) -> None:
        line = f"[{time.strftime('%H:%M:%S')}] {message}"
        print(line, file=sys.stderr, flush=True)
        self.file.write(line + "\n")
        self.file.flush()


def run(cmd: list[str], log: Log, **kwargs) -> subprocess.CompletedProcess:
    log("$ " + " ".join(str(c) for c in cmd))
    return subprocess.run([str(c) for c in cmd], check=True, text=True, **kwargs)


def tree_digest(paths: list[pathlib.Path]) -> str:
    h = hashlib.sha256()
    for base in paths:
        files = sorted(p for p in base.rglob("*") if p.is_file() and "target" not in p.parts) if base.is_dir() else [base]
        for f in files:
            h.update(str(f.relative_to(base.parent)).encode())
            h.update(f.read_bytes())
    return h.hexdigest()


def build_memd(checkout: pathlib.Path, out_dir: pathlib.Path, log: Log) -> pathlib.Path:
    tm = (checkout / "vendor/tinymemory/crates").resolve()
    manifest = (MEMD_SRC / "Cargo.toml").read_text(encoding="utf-8")
    manifest = manifest.replace("../../vendor/openhuman/vendor/tinymemory/crates", str(tm))
    manifest = manifest.replace('path = "src/main.rs"', f'path = "{(MEMD_SRC / "src/main.rs").resolve()}"')
    build_dir = out_dir / "memd-build"
    build_dir.mkdir(parents=True, exist_ok=True)
    (build_dir / "Cargo.toml").write_text(manifest, encoding="utf-8")
    lock = MEMD_SRC / "Cargo.lock"
    if lock.exists():
        shutil.copy(lock, build_dir / "Cargo.lock")
    target = ROOT / "target/memd"
    with (out_dir / "memd-build.log").open("w") as out:
        run(["cargo", "build", "--manifest-path", build_dir / "Cargo.toml", "--target-dir", target], log, stdout=out, stderr=subprocess.STDOUT)
    binary = out_dir / "lme2-memd"
    shutil.copy(target / "debug/lme2-memd", binary)
    return binary


def http_json(url: str, payload=None, timeout: float = 30) -> dict:
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={"content-type": "application/json"}, method="GET" if payload is None else "POST")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def docker_gateway(gateway: str) -> str:
    return gateway.replace("127.0.0.1", "host.docker.internal").replace("localhost", "host.docker.internal")


def start_cortex(name: str, data_dir: pathlib.Path, gateway: str, log: Log) -> str:
    inference = f"{docker_gateway(gateway)}/__tag/ingest/cortex/v1"
    env = {
        "CORTEX_API_KEY": "lme2-local", "CORTEX_DEPLOYMENT_PRESET": "dev_local",
        "CORTEX_EMBEDDING_URL": inference, "CORTEX_EMBEDDING_API_KEY": "dummy", "CORTEX_EMBEDDING_MODEL": EMBED,
        "CORTEX_EMBEDDING_DIMS": "1536", "OPENAI_API_KEY": "dummy", "CORTEX_LLM_URL": inference,
        "CORTEX_ENTITY_API_KEY": "dummy", "CORTEX_LLM_MODEL": MODEL, "CORTEX_ENRICHMENT_URL": inference,
        "CORTEX_ENRICHMENT_API_KEY": "dummy", "CORTEX_ENRICHMENT_MODEL": MODEL, "CORTEX_ENRICHMENT_DELAY_SECONDS": "0",
        "CORTEX_ANSWER_PROVIDER": "openai", "CORTEX_ANSWER_URL": inference, "CORTEX_ANSWER_API_KEY": "dummy",
        "CORTEX_ANSWER_MODEL": MODEL, "CORTEX_VERIFIER_URL": inference, "CORTEX_VERIFIER_API_KEY": "dummy",
        "CORTEX_VERIFIER_MODEL": MODEL, "CORTEX_VERIFIER_MAX_TOKENS": "4096", "CORTEX_ENTITY_GRAPH": "1",
        "CORTEX_V1_LAYERS_AUTO": "1", "CORTEX_AUTO_ROUTE": "1", "CORTEX_CONSOLIDATION_MIN_AGE_HOURS": "0",
    }
    subprocess.run(["docker", "rm", "-f", name], capture_output=True)
    cmd = ["docker", "run", "-d", "--name", name, "-p", "127.0.0.1::3141", "-v", f"{data_dir}:/data/cortex",
           "-v", f"{CORTEX_TOML}:/data/cortex.toml:ro"]
    for k, v in env.items():
        cmd += ["-e", f"{k}={v}"]
    run(cmd + [CORTEX_IMAGE, "3141", "/data"], log, capture_output=True)
    port = subprocess.check_output(["docker", "port", name, "3141/tcp"], text=True).strip().splitlines()[0].rsplit(":", 1)[1]
    url = f"http://127.0.0.1:{port}"
    for _ in range(180):
        try:
            urllib.request.urlopen(f"{url}/v1/admin/ready", timeout=2)
            log(f"cortex ready at {url}")
            return url
        except Exception:
            time.sleep(1)
    raise RuntimeError("CortexDB never became ready")


def subset_inputs(subset: dict, limit: int | None, out: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path, dict]:
    wanted = {}
    for split in ("dev", "test"):
        rows = subset[split][:limit] if limit else subset[split]
        for q in rows:
            wanted[q["id"]] = (split, q["sha256"])
    lines = []
    for line in (DATA / "questions.jsonl").read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row["id"] in wanted:
            if hashlib.sha256(line.encode()).hexdigest() != wanted[row["id"]][1]:
                raise RuntimeError(f"question {row['id']} changed since the subset was frozen")
            lines.append(line)
    if len(lines) != len(wanted):
        raise RuntimeError("subset questions missing from questions.jsonl")
    haystacks = json.loads((DATA / "haystacks/lme_v2_small.json").read_text(encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    qpath, hpath = out / "questions.jsonl", out / "haystack.json"
    qpath.write_text("\n".join(lines) + "\n", encoding="utf-8")
    hpath.write_text(json.dumps({qid: haystacks[qid] for qid in wanted}), encoding="utf-8")
    return qpath, hpath, {qid: split for qid, (split, _) in wanted.items()}


def summarize(per_question: pathlib.Path, splits: dict, out: pathlib.Path) -> dict:
    records = [json.loads(l) for l in per_question.read_text(encoding="utf-8").splitlines() if l.strip()]
    summary = {"questions": len(records), "splits": {}}
    failures = []
    for split in ("dev", "test"):
        rows = [r for r in records if splits.get(r["question_id"]) == split]
        by_type = {}
        for r in rows:
            by_type.setdefault(r["question_type"], []).append(r["score"])
        summary["splits"][split] = {
            "n": len(rows),
            "accuracy": sum(r["score"] for r in rows) / len(rows) if rows else None,
            "by_type": {t: {"n": len(v), "accuracy": sum(v) / len(v)} for t, v in sorted(by_type.items())},
            "scores": {r["question_id"]: r["score"] for r in rows},
            "pack_tokens_mean": sum(r.get("memory_context_token_count") or 0 for r in rows) / len(rows) if rows else None,
        }
        if split == "dev":
            for r in rows:
                if r["score"] < 1:
                    pack = "\n".join(i["value"] for i in r.get("memory_context") or [] if i.get("type") == "text")
                    failures.append({"question_id": r["question_id"], "question_type": r["question_type"],
                                     "question": r["question_text"], "model_answer": r["response_parsed_boxed"],
                                     "answered_unknown": r["is_unknown"], "retrieved_pack": pack})
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    with (out / "dev_failures.jsonl").open("w", encoding="utf-8") as f:
        for row in failures:
            f.write(json.dumps(row) + "\n")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--checkout", default=str(ROOT / "vendor/openhuman"))
    parser.add_argument("--label", required=True)
    parser.add_argument("--run-dir", required=True)
    parser.add_argument("--gateway", required=True)
    parser.add_argument("--engine", choices=["cortex", "reference"], default="cortex")
    parser.add_argument("--limit", type=int, default=None, help="first N questions of each split (smoke runs)")
    parser.add_argument("--concurrency", type=int, default=8)
    args = parser.parse_args()

    checkout = pathlib.Path(args.checkout).resolve()
    out = (pathlib.Path(args.run_dir) / args.label).resolve()
    out.mkdir(parents=True, exist_ok=True)
    log = Log(out / "eval.log")
    subset = json.loads((ROOT / "campaigns/lme2/subset.json").read_text(encoding="utf-8"))
    qpath, hpath, splits = subset_inputs(subset, args.limit, out / "inputs")

    log(f"building lme2-memd against {checkout}")
    memd = build_memd(checkout, out, log)

    haystack_ids = json.loads(hpath.read_text())[next(iter(splits))]
    key_material = json.dumps({
        "haystack": haystack_ids, "engine": args.engine, "image": CORTEX_IMAGE,
        "write_path": tree_digest([checkout / p for p in WRITE_PATH] + [MEMD_SRC / "src/convert.rs", CORTEX_TOML]),
    }, sort_keys=True)
    key = hashlib.sha256(key_material.encode()).hexdigest()[:24]
    cache = ROOT / "runs/ingest-cache" / key
    hit = args.engine == "cortex" and (cache / "COMPLETE").exists()
    log(f"ingest cache {key}: {'hit' if hit else 'miss'}")

    cortex_name = f"ohrsi-{args.label}-cortex".replace("/", "-")
    procs = []
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "TMPDIR")}
    try:
        if args.engine == "cortex":
            data_dir = out / "cortex-data"
            shutil.rmtree(data_dir, ignore_errors=True)
            if hit:
                run(["cp", "-cR", cache / "cortex-data", data_dir], log)
            else:
                data_dir.mkdir(parents=True)
            env["CORTEX_DB_URL"] = start_cortex(cortex_name, data_dir, args.gateway, log)
            env["CORTEX_API_KEY"] = "lme2-local"
        memd_log = (out / "memd.log").open("w")
        memd_proc = subprocess.Popen([memd, "--engine", args.engine, "--namespace", f"lme2-{subset['domain']}"],
                                     env=env, stdout=subprocess.PIPE, stderr=memd_log, text=True)
        procs.append(memd_proc)
        line = memd_proc.stdout.readline().strip()
        if not line.startswith("listening on "):
            raise RuntimeError(f"lme2-memd failed to start: {line!r}")
        memd_url = line.removeprefix("listening on ")
        log(f"memd at {memd_url}")

        config = {"memory_type": "oh_tinymemory", "memory_params": {"memd_url": docker_gateway(memd_url), "skip_insert": hit}}
        (out / "memory_config.json").write_text(json.dumps(config, indent=2), encoding="utf-8")
        harness_out = out / "harness"
        shutil.rmtree(harness_out, ignore_errors=True)
        gw = docker_gateway(args.gateway)
        cmd = [
            "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
            "-v", f"{MOUNT_ROOT}:{MOUNT_ROOT}", "-w", str(ROOT),
            "-e", "OPENAI_API_KEY=dummy", "-e", f"HF_HOME={BENCH / '.cache/hf'}",
            HARNESS_IMAGE, str(VENV_PY), str(ROOT / "adapters/lme2/run_harness.py"), str(OFFICIAL),
            "--domain", subset["domain"], "--questions-path", str(qpath), "--haystack-path", str(hpath),
            "--trajectories-path", str(DATA / "trajectories.jsonl"), "--memory-config-path", str(out / "memory_config.json"),
            "--output-dir", str(harness_out), "--model", MODEL, "--base-url", f"{gw}/__tag/reader/harness/v1",
            "--max-completion-tokens", "4096", "--reader-max-concurrent-requests", str(args.concurrency),
            "--reasoning-effort", "low", "--timeout-seconds", "600",
            "--evaluator-model", MODEL, "--evaluator-base-url", f"{gw}/__tag/judge/harness/v1",
            "--evaluator-reasoning-effort", "low", "--evaluator-max-completion-tokens", "2048",
            "--evaluator-timeout-seconds", "600",
        ]
        with (out / "harness.log").open("w") as hl:
            run(cmd, log, stdout=hl, stderr=subprocess.STDOUT)
        if args.engine == "cortex" and not hit:
            run(["docker", "stop", cortex_name], log, capture_output=True)
            tmp = cache.with_name(key + ".tmp")
            shutil.rmtree(tmp, ignore_errors=True)
            tmp.mkdir(parents=True)
            run(["cp", "-cR", out / "cortex-data", tmp / "cortex-data"], log)
            (tmp / "COMPLETE").write_text(key_material, encoding="utf-8")
            if cache.exists():
                shutil.rmtree(cache)
            tmp.rename(cache)
            log(f"cached ingest as {key}")
    finally:
        for p in procs:
            p.terminate()
        if args.engine == "cortex":
            subprocess.run(["docker", "logs", cortex_name], stdout=(out / "cortex.log").open("w"), stderr=subprocess.STDOUT)
            subprocess.run(["docker", "rm", "-f", cortex_name], capture_output=True)
        shutil.rmtree(out / "cortex-data", ignore_errors=True)

    per_question = next(harness_out.rglob("per_question.jsonl"))
    summary = summarize(per_question, splits, out)
    summary.update({"label": args.label, "checkout": str(checkout), "ingest_cache": key, "ingest_cache_hit": hit})
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for split in ("dev", "test"):
        acc = summary["splits"][split]["accuracy"]
        if acc is not None:
            print(f"METRIC lme2_{split}_accuracy={acc:.6f}")


if __name__ == "__main__":
    main()
