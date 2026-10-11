#!/usr/bin/env python3
"""Score one TinyMemory checkout on the frozen LongMemEval-V2 subset.

tinyrsi runs this as the ``lme2`` scorer of ``loops/lme2-recall`` under its
step contract: ``$RSI_CANDIDATE`` is the TinyMemory worktree to measure,
``$RSI_OUT`` the empty output folder, ``$RSI_GATEWAY`` the run's spend
gateway and ``$RSI_LABEL`` the candidate id. For manual use every input is
also a flag:

    lme2.py --candidate <tinymemory dir> --out <dir> --gateway http://127.0.0.1:<port>
            [--label <name>] [--engine cortex|reference] [--limit N] [--concurrency N]

``--limit``, ``--engine`` and ``--concurrency`` fall back to ``LME2_LIMIT``,
``LME2_ENGINE`` and ``LME2_CONCURRENCY`` (set them in ``rsi.json``'s ``env``
for a smoke run). Steps, each logged to ``<out>/eval.log``:

1. Build ``lme2-memd`` against the checkout's own TinyMemory crates.
2. Start a fresh CortexDB container whose LLM and embedding traffic goes
   through the spend gateway (``/__tag/ingest/cortex``). Its data directory
   is restored from the ingest cache when the haystack *and* the write path
   (TinyMemory's api/integrations sources, the adapter's converter, the
   CortexDB image and config) are unchanged; otherwise it ingests and the
   result is cached for later candidates. A restored ingest that still had
   enrichment queued replaces the cache once it has settled.
3. Run the unmodified official harness in its Linux container with the
   ``oh_tinymemory`` backend, GLM 5.3 Flash as reader and judge, both
   through the gateway (``/__tag/reader/harness``, ``/__tag/judge/harness``).
4. Score per split and write ``summary.json``, ``dev_failures.jsonl`` (dev
   only: question, the model's answer, verdict, retrieved pack — never the
   gold answer), ``result.json`` for tinyrsi, and print
   ``METRIC lme2_<split>_accuracy=<x>`` lines.

Everything here runs locally; the only network egress is the gateway's.
"""

import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import time
import urllib.request

# loops/lme2-recall/scorers/lme2.py -> the oh-bench-rsi checkout.
ROOT = pathlib.Path(__file__).resolve().parents[3]
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
# From a linked worktree (`worktrees/<slug>`) that is the primary checkout,
# so the shared `benchmarks/.cache` resolves too.
MOUNT_ROOT = pathlib.Path(os.environ.get("LME2_MOUNT_ROOT", str(ROOT.parents[1] if ROOT.parent.name == "worktrees" else ROOT)))
# Enrichment throughput settings. CortexDB otherwise enqueues enrichment in
# 256-event scan chunks with low concurrency, which leaves most of a
# 1,700-event haystack unenriched for hours. One wave, sixteen workers.
CORTEX_INGEST_ENV = {
    "CORTEX_ENRICHMENT_SCAN_CHUNK": "4096",
    "CORTEX_ENRICHMENT_CONCURRENCY": "16",
    # No CORTEX_ENRICHMENT_BACKFILL: a restored snapshot must not re-enrich
    # (measured: a backfill restart re-dispatched 1,168 jobs). Backfill is
    # only for salvaging a live container with lme2_snapshot.py.
    # Cost bounds. Measured on this haystack, full enrichment costs ~$0.004
    # per event (~$7 for 1,737), more than one $5 run. TinyMemory recall
    # does not read the entity graph (45% of the calls), and fact extraction
    # needs the gist of an event, not a whole page dump: embeddings and
    # recall still index the full text.
    "CORTEX_ENTITY_GRAPH": "0",
    "CORTEX_ENRICHMENT_LONG_TEXT_MAX_BYTES": "3072",
    "CORTEX_ENRICHMENT_MAX_LLM_CALLS_PER_EVENT": "2",
}
# The ingest write path, relative to the TinyMemory checkout.
WRITE_PATH = [
    "crates/tinymemory-api",
    "crates/tinymemory-integrations",
]
# How many dev failures result.json carries, and how many of one type.
MAX_FAILURES = 30
FAILURES_PER_TYPE = 4
QUESTION_CHARS = 600
PACK_CHARS = 1500


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


def build_memd(tinymemory: pathlib.Path, out_dir: pathlib.Path, log: Log) -> pathlib.Path:
    tm = (tinymemory / "crates").resolve()
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


def ingest_key(tinymemory: pathlib.Path, haystack_ids: list[str], engine: str) -> tuple[str, str]:
    """The ingest cache key: haystack, engine, image, ingest env and write path."""
    material = json.dumps({
        "haystack": haystack_ids, "engine": engine, "image": CORTEX_IMAGE, "ingest_env": CORTEX_INGEST_ENV,
        "write_path": tree_digest([tinymemory / p for p in WRITE_PATH] + [MEMD_SRC / "src/convert.rs", CORTEX_TOML]),
    }, sort_keys=True)
    return hashlib.sha256(material.encode()).hexdigest()[:24], material


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
        "CORTEX_VERIFIER_MODEL": MODEL, "CORTEX_VERIFIER_MAX_TOKENS": "4096",
        "CORTEX_V1_LAYERS_AUTO": "1", "CORTEX_AUTO_ROUTE": "1", "CORTEX_CONSOLIDATION_MIN_AGE_HOURS": "0",
        **CORTEX_INGEST_ENV,
    }
    subprocess.run(["docker", "rm", "-f", name], capture_output=True)
    # CortexDB keeps all of its state (WAL, HNSW, facts, snapshots) under its
    # data root, `/data`. Persist the whole root, so a stopped container's
    # directory is a complete, restorable ingest; the config sits inside it.
    shutil.copy(CORTEX_TOML, data_dir / "cortex.toml")
    cmd = ["docker", "run", "-d", "--name", name, "-p", "127.0.0.1::3141", "-v", f"{data_dir}:/data"]
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


BACKLOG = re.compile(r"enrichment Backlog \{ pending: (\d+),")


def drained_on_restore(memd_log: str) -> bool:
    """Whether a restored ingest still had enrichment pending when memd first
    polled it. Such a snapshot drains its queue (paid model calls) on every
    restore, so once it has settled it replaces the cached copy."""
    first = BACKLOG.search(memd_log)
    return bool(first and int(first.group(1)) > 0)


def store_cache(data_dir: pathlib.Path, cache: pathlib.Path, key_material: str, log: Log) -> None:
    """Atomically replace ``cache`` with a copy-on-write clone of a stopped
    CortexDB data directory."""
    tmp = cache.with_name(cache.name + ".tmp")
    shutil.rmtree(tmp, ignore_errors=True)
    tmp.mkdir(parents=True)
    run(["cp", "-cR", data_dir, tmp / "cortex-data"], log)
    (tmp / "COMPLETE").write_text(key_material, encoding="utf-8")
    if cache.exists():
        shutil.rmtree(cache)
    tmp.rename(cache)


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


def summarize(records: list[dict], splits: dict) -> tuple[dict, list[dict]]:
    """Per-split accuracy and the dev failures, from the harness's per-question rows.

    A failure row carries the question, the model's answer, whether it
    answered "unknown" and the recall pack — never the gold answer.
    """
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
    return summary, failures


def failure_text(row: dict) -> str:
    """The Markdown the hive sees for one failed dev question."""
    pack = row["retrieved_pack"][:PACK_CHARS] or "(empty pack)"
    unknown = " [answered unknown]" if row["answered_unknown"] else ""
    return "\n".join([
        f"Question: {(row['question'] or '').strip()[:QUESTION_CHARS]}",
        f"Model answer: {row['model_answer'] or '(none)'}{unknown}",
        "Recall pack the model saw (truncated):",
        "```",
        pack,
        "```",
    ])


def metric_name(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", text.lower()).strip("_")


def build_result(summary: dict, failures: list[dict]) -> dict:
    """tinyrsi's ``result.json``: dev accuracy as the score, per-item 0/1
    scores for both splits, a sample of dev failures, and dev/test metrics."""
    dev, test = summary["splits"]["dev"], summary["splits"]["test"]
    if dev["accuracy"] is None:
        raise RuntimeError("no dev questions were scored")
    shown, per_type = [], {}
    for row in failures:
        kind = row["question_type"]
        if per_type.get(kind, 0) >= FAILURES_PER_TYPE:
            continue
        per_type[kind] = per_type.get(kind, 0) + 1
        shown.append({"id": row["question_id"], "type": kind, "text": failure_text(row)})
        if len(shown) >= MAX_FAILURES:
            break
    metrics = {"dev_accuracy": dev["accuracy"]}
    if test["accuracy"] is not None:
        metrics["test_accuracy"] = test["accuracy"]
    if dev["pack_tokens_mean"] is not None:
        metrics["dev_pack_tokens_mean"] = dev["pack_tokens_mean"]
    for kind, row in dev["by_type"].items():
        metrics[f"dev_accuracy_{metric_name(kind)}"] = row["accuracy"]
    return {
        "score": dev["accuracy"],
        "splits": {split: {"items": {qid: float(s) for qid, s in summary["splits"][split]["scores"].items()}}
                   for split in ("dev", "test")},
        "failures": shown,
        "metrics": metrics,
    }


def env_int(name: str) -> int | None:
    value = os.environ.get(name, "").strip()
    return int(value) if value else None


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--candidate", default=os.environ.get("RSI_CANDIDATE") or str(ROOT / "vendor/openhuman/vendor/tinymemory"),
                        help="TinyMemory checkout to measure (default: $RSI_CANDIDATE, else the vendored one)")
    parser.add_argument("--out", default=os.environ.get("RSI_OUT"), help="output folder (default: $RSI_OUT)")
    parser.add_argument("--gateway", default=os.environ.get("RSI_GATEWAY"), help="spend gateway base (default: $RSI_GATEWAY)")
    parser.add_argument("--label", default=os.environ.get("RSI_LABEL") or "manual")
    parser.add_argument("--engine", choices=["cortex", "reference"], default=os.environ.get("LME2_ENGINE") or "cortex")
    parser.add_argument("--limit", type=int, default=env_int("LME2_LIMIT"), help="first N questions of each split (smoke runs)")
    parser.add_argument("--concurrency", type=int, default=env_int("LME2_CONCURRENCY") or 8)
    args = parser.parse_args(argv)
    if not args.out:
        parser.error("--out or $RSI_OUT is required")
    if not args.gateway:
        parser.error("--gateway or $RSI_GATEWAY is required")
    return args


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    tinymemory = pathlib.Path(args.candidate).resolve()
    out = pathlib.Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    log = Log(out / "eval.log")
    subset = json.loads((ROOT / "campaigns/lme2/subset.json").read_text(encoding="utf-8"))
    qpath, hpath, splits = subset_inputs(subset, args.limit, out / "inputs")

    log(f"building lme2-memd against {tinymemory}")
    memd = build_memd(tinymemory, out, log)

    haystack_ids = json.loads(hpath.read_text())[next(iter(splits))]
    key, key_material = ingest_key(tinymemory, haystack_ids, args.engine)
    cache = ROOT / "runs/ingest-cache" / key
    hit = args.engine == "cortex" and (cache / "COMPLETE").exists()
    log(f"ingest cache {key}: {'hit' if hit else 'miss'}")

    cortex_name = "lme2-" + re.sub(r"[^A-Za-z0-9_.-]", "-", args.label) + "-cortex"
    procs = []
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "TMPDIR")}
    env["MEMD_SETTLE_SECS"] = os.environ.get("MEMD_SETTLE_SECS", "10800")
    succeeded = False
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
        # tinyrsi's step folder lives in its run store, outside the checkout.
        mounts = ["-v", f"{MOUNT_ROOT}:{MOUNT_ROOT}"]
        if not out.is_relative_to(MOUNT_ROOT):
            mounts += ["-v", f"{out}:{out}"]
        cmd = [
            "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
            *mounts, "-w", str(ROOT),
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
        memd_log.flush()
        drained = hit and drained_on_restore((out / "memd.log").read_text(encoding="utf-8", errors="replace"))
        if args.engine == "cortex" and (not hit or drained):
            run(["docker", "stop", cortex_name], log, capture_output=True)
            store_cache(out / "cortex-data", cache, key_material, log)
            log(f"{'re-cached settled' if hit else 'cached'} ingest as {key}")
        succeeded = True
    finally:
        for p in procs:
            p.terminate()
        if args.engine == "cortex":
            subprocess.run(["docker", "logs", cortex_name], stdout=(out / "cortex.log").open("w"), stderr=subprocess.STDOUT)
            subprocess.run(["docker", "rm", "-f", cortex_name], capture_output=True)
        # A failed run keeps its data directory: a paid ingest is recoverable
        # with lme2_snapshot.py. Only a finished run (cached or a cache hit)
        # discards its working copy.
        if succeeded:
            shutil.rmtree(out / "cortex-data", ignore_errors=True)
        else:
            log(f"evaluation failed; keeping {out / 'cortex-data'} for lme2_snapshot.py")

    per_question = next(harness_out.rglob("per_question.jsonl"))
    records = [json.loads(l) for l in per_question.read_text(encoding="utf-8").splitlines() if l.strip()]
    summary, failures = summarize(records, splits)
    with (out / "dev_failures.jsonl").open("w", encoding="utf-8") as f:
        for row in failures:
            f.write(json.dumps(row) + "\n")
    head = subprocess.run(["git", "-C", str(tinymemory), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "-C", str(tinymemory), "status", "--porcelain"], capture_output=True, text=True).stdout.strip())
    summary.update({"label": args.label, "checkout": str(tinymemory), "ingest_cache": key, "ingest_cache_hit": hit,
                    "tinymemory_head": head, "tinymemory_dirty": dirty, "limit": args.limit})
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    (out / "result.json").write_text(json.dumps(build_result(summary, failures), indent=2) + "\n", encoding="utf-8")
    for split in ("dev", "test"):
        acc = summary["splits"][split]["accuracy"]
        if acc is not None:
            print(f"METRIC lme2_{split}_accuracy={acc:.6f}")


if __name__ == "__main__":
    main()
