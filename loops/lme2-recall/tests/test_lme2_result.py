"""Unit tests for the pure parts of ``scorers/lme2.py``: summarising the
harness's per-question rows and building tinyrsi's ``result.json``."""

import json
import pathlib
import sys
import unittest
import unittest.mock

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "scorers"))

import lme2  # noqa: E402

SPLITS = {"d1": "dev", "d2": "dev", "d3": "dev", "t1": "test", "t2": "test"}


def fixture() -> list[dict]:
    text = (HERE / "fixtures/per_question.jsonl").read_text(encoding="utf-8")
    return [json.loads(l) for l in text.splitlines() if l.strip()]


class BuildResultTest(unittest.TestCase):
    def setUp(self) -> None:
        self.records = fixture()
        self.summary, self.failures = lme2.summarize(self.records, SPLITS)
        self.result = lme2.build_result(self.summary, self.failures)

    def test_score_is_dev_accuracy(self) -> None:
        self.assertAlmostEqual(self.result["score"], 1 / 3)

    def test_splits_carry_per_question_scores(self) -> None:
        self.assertEqual(self.result["splits"]["dev"]["items"], {"d1": 0.0, "d2": 1.0, "d3": 0.0})
        self.assertEqual(self.result["splits"]["test"]["items"], {"t1": 1.0, "t2": 0.0})

    def test_metrics_report_dev_pack_and_per_type_and_keep_test_private(self) -> None:
        metrics = self.result["metrics"]
        self.assertAlmostEqual(metrics["dev_accuracy"], 1 / 3)
        self.assertNotIn("test_accuracy", metrics)
        self.assertAlmostEqual(self.result["private"]["test_accuracy"], 0.5)
        self.assertAlmostEqual(metrics["dev_pack_tokens_mean"], 200 / 3)
        self.assertAlmostEqual(metrics["dev_accuracy_temporal"], 0.5)
        self.assertAlmostEqual(metrics["dev_accuracy_single_hop"], 0.0)

    def test_failures_are_dev_only_with_id_and_type(self) -> None:
        self.assertEqual([(f["id"], f["type"]) for f in self.result["failures"]],
                         [("d1", "temporal"), ("d3", "single-hop")])

    def test_result_never_contains_a_gold_answer_or_test_detail(self) -> None:
        failures = json.dumps(self.result["failures"])
        for row in self.records:
            self.assertNotIn(row["answer"], failures)
        for leaked in ("Held-out", "held-out pack"):
            self.assertNotIn(leaked, failures)
        self.assertNotIn("answer", {k for f in self.failures for k in f})

    def test_result_is_json_with_a_finite_score(self) -> None:
        decoded = json.loads(json.dumps(self.result))
        self.assertEqual(set(decoded), {"score", "splits", "failures", "metrics", "private"})

    def test_failures_are_capped_per_type_and_in_total(self) -> None:
        many = [dict(self.failures[0], question_id=f"x{i}") for i in range(10)]
        many += [dict(self.failures[1], question_id=f"y{i}", question_type=f"t{i}") for i in range(40)]
        shown = lme2.build_result(self.summary, many)["failures"]
        self.assertEqual(sum(f["type"] == "temporal" for f in shown), lme2.FAILURES_PER_TYPE)
        self.assertEqual(len(shown), lme2.MAX_FAILURES)

    def test_no_dev_questions_is_an_error(self) -> None:
        summary, failures = lme2.summarize(self.records, {"t1": "test"})
        with self.assertRaises(RuntimeError):
            lme2.build_result(summary, failures)


class FailureTextTest(unittest.TestCase):
    def row(self, **overrides) -> dict:
        row = {"question_id": "q", "question_type": "temporal", "question": "What did I buy?",
               "model_answer": "socks", "answered_unknown": False, "retrieved_pack": "bought socks"}
        row.update(overrides)
        return row

    def test_shows_question_answer_and_fenced_pack(self) -> None:
        text = lme2.failure_text(self.row())
        self.assertEqual(text.splitlines(), [
            "Question: What did I buy?",
            "Model answer: socks",
            "Recall pack the model saw (truncated):",
            "```",
            "bought socks",
            "```",
        ])

    def test_marks_unknown_answers_and_empty_packs(self) -> None:
        text = lme2.failure_text(self.row(model_answer=None, answered_unknown=True, retrieved_pack=""))
        self.assertIn("Model answer: (none) [answered unknown]", text)
        self.assertIn("(empty pack)", text)

    def test_truncates_question_and_pack(self) -> None:
        text = lme2.failure_text(self.row(question="q" * 2000, retrieved_pack="p" * 5000))
        self.assertIn("Question: " + "q" * lme2.QUESTION_CHARS + "\n", text)
        self.assertNotIn("q" * (lme2.QUESTION_CHARS + 1), text)
        self.assertIn("p" * lme2.PACK_CHARS + "\n```", text)
        self.assertNotIn("p" * (lme2.PACK_CHARS + 1), text)


class ArgumentsTest(unittest.TestCase):
    def test_step_environment_supplies_the_defaults(self) -> None:
        env = {"RSI_CANDIDATE": "/tm", "RSI_OUT": "/out", "RSI_GATEWAY": "http://127.0.0.1:9",
               "RSI_LABEL": "e1-implementer-a", "LME2_LIMIT": "5", "LME2_ENGINE": "reference",
               "LME2_CONCURRENCY": "2"}
        with unittest.mock.patch.dict(lme2.os.environ, env, clear=True):
            args = lme2.parse_args([])
        self.assertEqual((args.candidate, args.out, args.gateway, args.label), ("/tm", "/out", "http://127.0.0.1:9", "e1-implementer-a"))
        self.assertEqual((args.limit, args.engine, args.concurrency), (5, "reference", 2))

    def test_out_and_gateway_are_required(self) -> None:
        with unittest.mock.patch.dict(lme2.os.environ, {}, clear=True), \
                unittest.mock.patch("sys.stderr"), self.assertRaises(SystemExit):
            lme2.parse_args([])


if __name__ == "__main__":
    unittest.main()


class SettledOnRestoreTest(unittest.TestCase):
    """A restored ingest that still had enrichment queued is re-cached once it
    settles, so the next score does not pay to drain the same queue again."""

    def test_a_restore_with_pending_enrichment_needs_recaching(self) -> None:
        log = ("lme2-memd: enrichment Backlog { pending: 380, queued: 1168, processed: 1833 } stored=0\n"
               "lme2-memd: enrichment Backlog { pending: 0, queued: 1168, processed: 2213 } stored=0\n")
        self.assertTrue(lme2.drained_on_restore(log))

    def test_a_settled_restore_does_not(self) -> None:
        log = "lme2-memd: enrichment Backlog { pending: 0, queued: 1168, processed: 2213 } stored=0\n"
        self.assertFalse(lme2.drained_on_restore(log))
        self.assertFalse(lme2.drained_on_restore(""))


class CortexEnvTest(unittest.TestCase):
    """CortexDB's model calls go through the spend gateway under the loop's
    chat model; only embeddings may go straight to a provider, named by
    ``LME2_EMBEDDING_URL`` and ``LME2_EMBEDDING_KEY_ENV``, since the cached
    index needs ``text-embedding-3-small`` and the gateway's router may not
    serve it."""

    GATEWAY = "http://127.0.0.1:9000"
    TAGGED = "http://host.docker.internal:9000/__tag/ingest/cortex/v1"

    def test_every_call_goes_through_the_gateway_by_default(self) -> None:
        env, secrets = lme2.cortex_env(self.GATEWAY, model="flash", embedding=None)
        for name in ("CORTEX_EMBEDDING_URL", "CORTEX_LLM_URL", "CORTEX_ENRICHMENT_URL",
                     "CORTEX_ANSWER_URL", "CORTEX_VERIFIER_URL"):
            self.assertEqual(env[name], self.TAGGED, name)
        for name in ("CORTEX_LLM_MODEL", "CORTEX_ENRICHMENT_MODEL", "CORTEX_ANSWER_MODEL", "CORTEX_VERIFIER_MODEL"):
            self.assertEqual(env[name], "flash", name)
        self.assertEqual(env["CORTEX_EMBEDDING_MODEL"], lme2.EMBED)
        self.assertEqual(secrets, [])

    def test_a_direct_embedding_route_keeps_its_key_off_the_command_line(self) -> None:
        route = lme2.EmbeddingRoute(url="https://openrouter.ai/api/v1", key_env="OPENROUTER_API_KEY")
        env, secrets = lme2.cortex_env(self.GATEWAY, model="flash", embedding=route)
        self.assertEqual(env["CORTEX_EMBEDDING_URL"], "https://openrouter.ai/api/v1")
        self.assertEqual(env["CORTEX_ENRICHMENT_URL"], self.TAGGED)
        self.assertEqual(secrets, ["CORTEX_EMBEDDING_API_KEY"])
        self.assertNotIn("CORTEX_EMBEDDING_API_KEY", env)

    def test_settings_come_from_the_environment(self) -> None:
        self.assertEqual(lme2.chat_model({}), lme2.MODEL)
        self.assertEqual(lme2.chat_model({"LME2_MODEL": "flash"}), "flash")
        self.assertIsNone(lme2.embedding_route({}))
        self.assertEqual(
            lme2.embedding_route({"LME2_EMBEDDING_URL": "https://e/v1", "LME2_EMBEDDING_KEY_ENV": "K"}),
            lme2.EmbeddingRoute("https://e/v1", "K"))
        with self.assertRaises(ValueError):
            lme2.embedding_route({"LME2_EMBEDDING_URL": "https://e/v1"})


class BuildEnvTest(unittest.TestCase):
    """The memd build runs with a short ``TMPDIR``: sccache puts its server
    socket there, and a tinyrsi step's own temp folder is too long for a Unix
    socket path."""

    def test_a_long_step_tmpdir_is_replaced_by_a_short_one(self) -> None:
        long_tmp = "/Users/someone/.tinyrsi/runs/lme2-recall/score-incumbent-20261012-002157/steps/score.lme2.0/tmp"
        env = lme2.build_env({"PATH": "/bin", "TMPDIR": long_tmp}, short_tmp="/var/folders/x/T/")
        self.assertEqual(env["TMPDIR"], "/var/folders/x/T/")
        self.assertEqual(env["PATH"], "/bin")

    def test_a_short_tmpdir_is_kept(self) -> None:
        env = lme2.build_env({"TMPDIR": "/tmp/x"}, short_tmp="/var/folders/x/T/")
        self.assertEqual(env["TMPDIR"], "/tmp/x")
