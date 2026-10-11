"""Retry a reader call whose reply came back empty.

The official harness raises ``RuntimeError("Model returned empty text")``
when the reader's reply has no text, and that one error aborts the whole
evaluation. A router that falls back between models (the LLM ladder's
``flash``) occasionally returns a reasoning-only reply with no content, so
``run_harness.py`` wraps the harness's reader call with ``retry_empty``: the
same request is sent again, up to ``attempts`` times in all. Every other
error, and a last empty reply, propagate unchanged.
"""

import functools

EMPTY_TEXT = "Model returned empty text"


def retry_empty(call, attempts: int = 3):
    """``call`` (an async function), retried while it raises the harness's
    empty-text error."""

    @functools.wraps(call)
    async def wrapped(*args, **kwargs):
        for attempt in range(1, attempts + 1):
            try:
                return await call(*args, **kwargs)
            except RuntimeError as error:
                if str(error) != EMPTY_TEXT or attempt == attempts:
                    raise

    return wrapped
