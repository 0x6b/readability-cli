import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))

from ranking import build_pairwise_examples


class RankingTest(unittest.TestCase):
    def test_builds_symmetric_pairs_against_best_candidate(self):
        features = [np.array([1.0, 2.0]), np.array([0.0, 1.0]), np.array([3.0, 0.0])]
        X, y, weights = build_pairwise_examples(features, [0.9, 0.2, 0.8])

        np.testing.assert_array_equal(X, [[1.0, 1.0], [-1.0, -1.0], [-2.0, 2.0], [2.0, -2.0]])
        np.testing.assert_array_equal(y, [1, 0, 1, 0])
        np.testing.assert_allclose(weights, [0.7, 0.7, 0.1, 0.1])

    def test_ignores_ambiguous_near_ties(self):
        X, y, weights = build_pairwise_examples(
            [np.array([1.0]), np.array([0.0])],
            [0.9, 0.86],
        )
        self.assertEqual(X.shape, (0, 1))
        self.assertEqual(len(y), 0)
        self.assertEqual(len(weights), 0)

    def test_rejects_mismatched_inputs(self):
        with self.assertRaisesRegex(ValueError, "equal length"):
            build_pairwise_examples([np.array([1.0])], [])


if __name__ == "__main__":
    unittest.main()
