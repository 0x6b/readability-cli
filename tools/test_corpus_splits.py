import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from corpus_splits import HOLDOUT_CASES, REGRESSION_CASES, corpus_family, corpus_split


CORPUS = Path(__file__).parent.parent / "tests" / "corpus"


class CorpusSplitsTest(unittest.TestCase):
    def test_all_source_cases_have_a_partition(self):
        cases = {
            path.stem
            for path in CORPUS.glob("*.html")
            if not path.stem.endswith(".expected")
        }
        assigned = {case: corpus_split(case) for case in cases}
        self.assertEqual(set(assigned.values()), {"train", "validation", "regression"})
        self.assertEqual(
            {case for case, split in assigned.items() if split == "regression"},
            REGRESSION_CASES,
        )

    def test_protected_sets_do_not_overlap(self):
        self.assertTrue(REGRESSION_CASES.isdisjoint(HOLDOUT_CASES))

    def test_families_do_not_cross_protected_partitions(self):
        cases = {
            path.stem
            for path in CORPUS.glob("*.html")
            if not path.stem.endswith(".expected")
        }
        family_splits: dict[str, set[str]] = {}
        for case in cases:
            family_splits.setdefault(corpus_family(case), set()).add(corpus_split(case))
        self.assertFalse(
            {family: splits for family, splits in family_splits.items() if len(splits) > 1}
        )


if __name__ == "__main__":
    unittest.main()
