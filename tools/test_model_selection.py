import sys
import unittest
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from corpus_splits import PROTECTED_SPLITS, corpus_family, corpus_split
from model_selection import assign_group_folds


CORPUS = Path(__file__).parent.parent / "tests" / "corpus"


class ModelSelectionTest(unittest.TestCase):
    def test_assignment_is_deterministic_and_keeps_families_together(self):
        cases = ["site-a-1", "site-a-2", "site-b", "site-c", "site-d"]
        assignments = assign_group_folds(cases, fold_count=3)
        self.assertEqual(assignments, assign_group_folds(reversed(cases), fold_count=3))
        self.assertEqual(assignments["site-a-1"], assignments["site-a-2"])

    def test_development_corpus_produces_balanced_folds(self):
        cases = [
            path.stem
            for path in CORPUS.glob("*.html")
            if not path.stem.endswith(".expected")
            and corpus_split(path.stem) not in PROTECTED_SPLITS
        ]
        assignments = assign_group_folds(cases)
        fold_sizes = Counter(assignments.values())
        largest_family = max(Counter(corpus_family(case) for case in cases).values())

        self.assertEqual(set(fold_sizes), set(range(5)))
        self.assertLessEqual(max(fold_sizes.values()) - min(fold_sizes.values()), largest_family)
        for family in {corpus_family(case) for case in cases}:
            self.assertEqual(
                len({assignments[case] for case in cases if corpus_family(case) == family}),
                1,
            )

    def test_rejects_single_fold(self):
        with self.assertRaisesRegex(ValueError, "at least 2"):
            assign_group_folds(["case"], fold_count=1)


if __name__ == "__main__":
    unittest.main()
