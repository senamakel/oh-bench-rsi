#!/usr/bin/env python3
"""Finish a detached ingest and store it in the ingest cache.

    lme2_snapshot.py --container <name> --data <cortex-data dir> --stored <n> \
                     --haystack <inputs/haystack.json> [--checkout vendor/openhuman]

Used when an evaluation is detached from a still-enriching CortexDB (so a
failure downstream cannot discard a paid ingest). Waits until every stored
event is enriched or dead-lettered and the queue is quiet, stops the
container, and clones its data directory into ``runs/ingest-cache/<key>``
under the same key ``lme2_eval.py`` computes.
"""

import argparse
import json
import pathlib
import subprocess
import sys
import time
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import lme2_eval as E  # noqa: E402


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--container", required=True)
    p.add_argument("--data", required=True)
    p.add_argument("--stored", type=int, required=True)
    p.add_argument("--haystack", required=True)
    p.add_argument("--checkout", default=str(E.ROOT / "vendor/openhuman"))
    a = p.parse_args()
    data = pathlib.Path(a.data).resolve()
    log = E.Log(data.parent / "snapshot.log")
    port = subprocess.check_output(["docker", "port", a.container, "3141/tcp"], text=True).split()[0].rsplit(":", 1)[1]
    quiet = 0
    while quiet < 3:
        req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/admin/usage", headers={"Authorization": "Bearer lme2-local"})
        u = json.loads(urllib.request.urlopen(req, timeout=30).read())
        cov, bl = u["enrichment_coverage"], u["enrichment_backlog"]
        done = cov["events_enriched"] + cov["events_failed"]
        log(f"enriched {cov['events_enriched']} failed {cov['events_failed']} pending {bl['jobs_pending']} queued {bl['jobs_queued_total']}")
        quiet = quiet + 1 if done >= a.stored and bl["jobs_pending"] == 0 else 0
        if quiet < 3:
            time.sleep(20)
    hay = json.loads(pathlib.Path(a.haystack).read_text())
    key, material = E.ingest_key(pathlib.Path(a.checkout).resolve(), hay[next(iter(hay))], "cortex")
    subprocess.run(["docker", "stop", a.container], check=True, capture_output=True)
    subprocess.run(["docker", "logs", a.container], stdout=(data.parent / "cortex.log").open("w"), stderr=subprocess.STDOUT)
    cache = E.ROOT / "runs/ingest-cache" / key
    tmp = cache.with_name(key + ".tmp")
    tmp.mkdir(parents=True)
    subprocess.run(["cp", "-cR", str(data), str(tmp / "cortex-data")], check=True)
    (tmp / "COMPLETE").write_text(material, encoding="utf-8")
    tmp.rename(cache)
    subprocess.run(["docker", "rm", "-f", a.container], capture_output=True)
    log(f"cached {key}")


if __name__ == "__main__":
    main()
