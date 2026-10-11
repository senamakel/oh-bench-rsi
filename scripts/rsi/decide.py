"""Promotion decision: paired, per-question, on the held-out test split.

Each test question is scored 0 or 1 for the incumbent and the candidate. Only
discordant pairs carry information. The candidate is accepted when it wins
more discordant pairs than it loses and an exact one-sided sign test rejects
"no difference" at ``alpha``. The estimate and its interval are the mean
paired difference with a Wilson-style normal interval on the discordant
fraction, reported in accuracy points (higher is better).
"""

import math


def sign_test_p(wins: int, losses: int) -> float:
    """One-sided exact binomial P(X >= wins | n = wins + losses, p = 0.5)."""
    n = wins + losses
    if n == 0:
        return 1.0
    return sum(math.comb(n, k) for k in range(wins, n + 1)) / 2**n


def decide(incumbent: dict[str, float], candidate: dict[str, float], alpha: float = 0.10) -> dict:
    """Compare two ``question_id -> score`` maps over their shared questions."""
    shared = sorted(set(incumbent) & set(candidate))
    if not shared:
        return {"decision": "inconclusive", "n": 0, "wins": 0, "losses": 0, "p": 1.0,
                "estimate": 0.0, "lower": 0.0, "upper": 0.0}
    diffs = [candidate[q] - incumbent[q] for q in shared]
    wins = sum(d > 0 for d in diffs)
    losses = sum(d < 0 for d in diffs)
    n = len(diffs)
    mean = sum(diffs) / n
    var = sum((d - mean) ** 2 for d in diffs) / max(n - 1, 1)
    half = 1.645 * math.sqrt(var / n)
    p = sign_test_p(wins, losses)
    if wins > losses and p < alpha:
        decision = "accept"
    elif losses > wins and sign_test_p(losses, wins) < alpha:
        decision = "reject"
    else:
        decision = "inconclusive"
    return {"decision": decision, "n": n, "wins": wins, "losses": losses, "p": p,
            "estimate": mean, "lower": mean - half, "upper": mean + half}
