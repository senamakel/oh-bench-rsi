"""Retry a reader call whose reply came back empty.

The official harness raises ``RuntimeError("Model returned empty text")``
when the reader's reply has no text, and that one error aborts the whole
evaluation. A router that falls back between models (the LLM ladder's
``flash``) returns such replies: a reasoning model can spend its whole
completion budget thinking and return no content. ``run_harness.py`` wraps
the harness's reader call with ``retry_empty``: the same request is sent
again, up to ``attempts`` times in all. A reply that is still empty scores
as no answer, exactly as the harness already scores a request the provider
refuses (``BadRequestError``): an empty response and zero usage. Every other
error propagates unchanged.
"""

import functools
import sys

EMPTY_TEXT = "Model returned empty text"
NO_USAGE = {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}


def retry_empty(call, attempts: int = 3):
    """``call`` (an async function), retried while it raises the harness's
    empty-text error; after ``attempts`` empty replies, ``("", NO_USAGE)``."""

    @functools.wraps(call)
    async def wrapped(*args, **kwargs):
        for _ in range(attempts):
            try:
                return await call(*args, **kwargs)
            except RuntimeError as error:
                if str(error) != EMPTY_TEXT:
                    raise
        print(f"Reader reply stayed empty after {attempts} attempts; scoring it as no answer.",
              file=sys.stderr, flush=True)
        return "", dict(NO_USAGE)

    return wrapped
