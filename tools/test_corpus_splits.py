import hashlib
import json
import sys
import unittest
from collections import Counter, defaultdict
from pathlib import Path
from urllib.parse import urlparse

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
        entries = manifest["cases"]
        manifest_cases = {entry["case"] for entry in entries}
        self.assertEqual(manifest_cases, HOLDOUT_CASES)
        self.assertEqual(len(manifest_cases), 50)
        self.assertEqual(
            len({corpus_family(case) for case in manifest_cases}),
            len(manifest_cases),
        )

        urls = [entry["url"] for entry in entries]
        hosts = [urlparse(url).hostname for url in urls]
        self.assertEqual(len(set(urls)), len(urls))
        self.assertEqual(len(set(hosts)), len(hosts))

        language_counts = Counter(entry["language"] for entry in entries)
        self.assertGreaterEqual(len(language_counts), 30)
        self.assertLessEqual(max(language_counts.values()), len(entries) * 0.4)

        holdout_paths = set()
        for case in manifest_cases:
            for suffix in (".html", ".expected.html"):
                path = CORPUS / f"{case}{suffix}"
                self.assertTrue(path.is_file())
                self.assertGreater(path.stat().st_size, 0)
                holdout_paths.add(path)

        paths_by_hash = defaultdict(list)
        for path in CORPUS.glob("*.html"):
            paths_by_hash[hashlib.sha256(path.read_bytes()).digest()].append(path)
        duplicate_holdouts = {
            path.name
            for paths in paths_by_hash.values()
            if len(paths) > 1
            for path in paths
            if path in holdout_paths
        }
        self.assertFalse(duplicate_holdouts)


if __name__ == "__main__":
    unittest.main()
