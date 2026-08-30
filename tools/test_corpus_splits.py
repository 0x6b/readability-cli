import json
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
        self.assertEqual(
            set(assigned.values()), {"train", "validation", "regression", "holdout"}
        )
        self.assertEqual(
            {case for case, split in assigned.items() if split == "regression"},
            REGRESSION_CASES,
        )
        self.assertEqual(
            {case for case, split in assigned.items() if split == "holdout"},
            HOLDOUT_CASES,
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

    def test_holdout_manifest_is_complete_and_blind(self):
        manifest = json.loads((Path(__file__).parent / "holdout_manifest.json").read_text())
        self.assertFalse(manifest["evaluated"])
        manifest_cases = {entry["case"] for entry in manifest["cases"]}
        self.assertEqual(manifest_cases, HOLDOUT_CASES)
        self.assertEqual(len(manifest_cases), 12)
        self.assertEqual(
            len({corpus_family(case) for case in manifest_cases}),
            len(manifest_cases),
        )
        for case in manifest_cases:
            self.assertTrue((CORPUS / f"{case}.html").is_file())
            self.assertTrue((CORPUS / f"{case}.expected.html").is_file())


if __name__ == "__main__":
    unittest.main()
