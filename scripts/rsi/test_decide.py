import unittest

from decide import decide, sign_test_p


class DecideTest(unittest.TestCase):
    def test_sign_test_matches_hand_values(self):
        self.assertAlmostEqual(sign_test_p(3, 0), 0.125)
        self.assertAlmostEqual(sign_test_p(0, 0), 1.0)
        self.assertAlmostEqual(sign_test_p(5, 1), 7 / 64)

    def test_accepts_a_clear_paired_improvement(self):
        inc = {f"q{i}": 0.0 for i in range(20)}
        cand = {f"q{i}": 1.0 if i < 8 else 0.0 for i in range(20)}
        result = decide(inc, cand)
        self.assertEqual(result["decision"], "accept")
        self.assertEqual((result["wins"], result["losses"]), (8, 0))
        self.assertGreater(result["lower"], 0)

    def test_noise_is_inconclusive(self):
        inc = {"a": 1.0, "b": 0.0, "c": 1.0, "d": 0.0}
        cand = {"a": 0.0, "b": 1.0, "c": 1.0, "d": 0.0}
        self.assertEqual(decide(inc, cand)["decision"], "inconclusive")

    def test_rejects_a_clear_regression(self):
        inc = {f"q{i}": 1.0 for i in range(10)}
        cand = {f"q{i}": 0.0 if i < 6 else 1.0 for i in range(10)}
        self.assertEqual(decide(inc, cand)["decision"], "reject")

    def test_compares_only_shared_questions(self):
        self.assertEqual(decide({"a": 1.0}, {"b": 1.0})["decision"], "inconclusive")


if __name__ == "__main__":
    unittest.main()
