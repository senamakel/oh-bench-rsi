"""OpenHuman memory as a LongMemEval-V2 ``Memory`` backend.

The official harness calls ``insert(trajectory)`` for every trajectory in
the haystack and then ``query(question)`` once per question. This backend
forwards both to ``lme2-memd``, which runs OpenHuman's TinyMemory lifecycle
(CortexDB engine, OpenHuman's default recall policy) on the host.

The backend never sees answers or question metadata: the harness passes it
only trajectories and the question text, and gold answers stay in the
harness. Before the first query it asks the daemon to *settle* — wait until
every stored item is readable and CortexDB enrichment has gone quiet — so
recall is measured against a fully indexed memory.

``memory_params``:

- ``memd_url``: the daemon's base URL as reachable from the harness.
- ``skip_insert``: true when the daemon's engine was restored from a cached
  ingest of this exact haystack and write path; inserts become no-ops.
"""

import json
import urllib.request

from memory_modules.memory import Memory, MemoryContextItem, register_memory


def _post(url: str, payload: object, timeout: float) -> dict:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


@register_memory
class OpenHumanTinyMemory(Memory):
    """Remember trajectories as agent turns; answer with OH's recall pack."""

    memory_type = "oh_tinymemory"

    def __init__(self, memory_params: dict[str, object]) -> None:
        super().__init__(memory_params)
        self._url = str(memory_params["memd_url"]).rstrip("/")
        self._skip_insert = bool(memory_params.get("skip_insert", False))
        self._settled = False

    def insert(self, trajectory: dict[str, object]) -> None:
        if self._skip_insert:
            return None
        _post(f"{self._url}/insert", trajectory, timeout=600)
        self._settled = False
        return None

    def query(self, query: str, query_image: str | None = None) -> list[MemoryContextItem]:
        if not self._settled:
            _post(f"{self._url}/settle", {}, timeout=3600)
            self._settled = True
        pack = _post(f"{self._url}/query", {"query": query}, timeout=600)
        markdown = str(pack.get("markdown") or "")
        return [{"type": "text", "value": markdown}] if markdown.strip() else []
