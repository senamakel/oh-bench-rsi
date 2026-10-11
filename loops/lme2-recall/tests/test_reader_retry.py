"""Unit tests for ``adapters/lme2/retry.py``: the runner retries a reader
call that came back empty, which the official harness otherwise turns into
an abort of the whole evaluation, and scores one that stays empty as no
answer."""

import asyncio
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "adapters" / "lme2"))

import retry  # noqa: E402

EMPTY = RuntimeError("Model returned empty text")


def flaky(results: list):
    """An async call returning (or raising) ``results`` in order."""
    calls = []

    async def call(*args):
        calls.append(args)
        outcome = results[len(calls) - 1]
        if isinstance(outcome, Exception):
            raise outcome
        return outcome

    return call, calls


class RetryEmptyTest(unittest.TestCase):
    def test_an_empty_reply_is_retried_until_one_has_text(self) -> None:
        call, calls = flaky([EMPTY, EMPTY, ("answer", {"completion_tokens": 3})])
        result = asyncio.run(retry.retry_empty(call, attempts=3)("client", "args", ["m"]))
        self.assertEqual(result, ("answer", {"completion_tokens": 3}))
        self.assertEqual(len(calls), 3)
        self.assertEqual(calls[0], ("client", "args", ["m"]))

    def test_a_reply_that_stays_empty_scores_as_no_answer(self) -> None:
        call, calls = flaky([EMPTY, EMPTY])
        text, usage = asyncio.run(retry.retry_empty(call, attempts=2)())
        self.assertEqual(text, "")
        self.assertEqual(usage, {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0})
        self.assertEqual(len(calls), 2)

    def test_other_errors_are_not_retried(self) -> None:
        call, calls = flaky([ValueError("bad request")])
        with self.assertRaises(ValueError):
            asyncio.run(retry.retry_empty(call, attempts=3)())
        self.assertEqual(len(calls), 1)
