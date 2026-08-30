import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from training_weights import candidate_sample_weight


class TrainingWeightsTest(unittest.TestCase):
    def test_positive_weight_increases_with_overlap(self):
        self.assertGreater(candidate_sample_weight(0.9, 0.5), candidate_sample_weight(0.5, 0.5))

    def test_ambiguous_negative_is_downweighted(self):
        self.assertLess(candidate_sample_weight(0.49, 0.5), candidate_sample_weight(0.0, 0.5))

    def test_negative_weight_range_preserves_class_balance(self):
        self.assertAlmostEqual(candidate_sample_weight(0.0, 0.5), 1.0)
        self.assertAlmostEqual(candidate_sample_weight(0.5 - 1e-9, 0.5), 2**-0.5)


if __name__ == "__main__":
    unittest.main()
