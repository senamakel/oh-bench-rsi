"""Unit tests for the pure parts of the gates: the leakage matcher and the
green gate's choice of crates to test."""

import json
import pathlib
import sys
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "gates"))

import green  # noqa: E402
import leakage  # noqa: E402

SUBSET = {"dev": [{"id": "q1"}], "test": [{"id": "q2"}]}
QUESTION = "on the shopping site, which shipping option did i pick for the blue kettle order last week?"
LINES = [
    json.dumps({"id": "q1", "question": QUESTION, "answer": "Express overnight; gift"}),
    json.dumps({"id": "q2", "question": "short one", "answer": "Saturday delivery"}),
    json.dumps({"id": "q9", "question": "not in the subset at all, a long enough question", "answer": "unrelated answer"}),
    "",
]


def diff(*added: str) -> str:
    lines = ["diff --git a/x.rs b/x.rs", "--- a/x.rs", "+++ b/x.rs", "@@ -1 +1 @@", "-old line"]
    return "\n".join(lines + [f"+{a}" for a in added]) + "\n"


class LeakageTest(unittest.TestCase):
    def setUp(self) -> None:
        self.questions, self.answers = leakage.benchmark_strings(SUBSET, LINES)

    def check(self, *added: str) -> dict:
        return leakage.leaks(leakage.added_text(diff(*added)), self.questions, self.answers)

    def test_reads_only_subset_questions_and_long_answer_fragments(self) -> None:
        self.assertEqual(self.questions, [QUESTION, "short one"])
        self.assertEqual(sorted(self.answers), ["express overnight", "saturday delivery"])

    def test_added_text_skips_headers_and_removed_lines(self) -> None:
        self.assertEqual(leakage.added_text(diff("New Line")), "new line")

    def test_a_clean_diff_passes(self) -> None:
        self.assertEqual(self.check("fn rank(items: &[Item]) -> Vec<Item> {", "// prefer recent evidence"), {})

    def test_flags_a_canary_case_insensitively(self) -> None:
        self.assertEqual(self.check('let boost = url.contains("Magento");'), {"canary": 1})

    def test_flags_a_gold_answer_fragment(self) -> None:
        self.assertEqual(self.check('if text == "Saturday Delivery" { score += 1 }'), {"gold answer": 1})

    def test_ignores_short_answer_fragments_and_other_questions(self) -> None:
        self.assertEqual(self.check("gift", "unrelated answer"), {})

    def test_flags_a_forty_character_question_window(self) -> None:
        self.assertEqual(self.check("// " + QUESTION[20:62]), {"question text": 1})

    def test_ignores_a_shorter_question_fragment(self) -> None:
        self.assertEqual(self.check(QUESTION[20:50]), {})


class GreenTest(unittest.TestCase):
    def test_touched_crates_are_the_sorted_crate_directories(self) -> None:
        paths = ["crates/tinymemory-tools/src/lib.rs", "crates/tinymemory-integrations/src/a/test.rs",
                 "crates/tinymemory-tools/Cargo.toml", "README.md", "crates/README.md"]
        self.assertEqual(green.touched_crates(paths), ["tinymemory-integrations", "tinymemory-tools"])

    def test_no_crate_changes_means_nothing_to_test(self) -> None:
        self.assertEqual(green.touched_crates(["docs/a.md"]), [])


if __name__ == "__main__":
    unittest.main()
