import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from import_blind_holdout import select_cases


class ImportBlindHoldoutTest(unittest.TestCase):
    def test_consumed_manifest_is_frozen_and_domain_diverse(self):
        tools = Path(__file__).parent
        manifest = json.loads(
            (tools / "external_regression_manifest_2026-08-31.json").read_text(
                encoding="utf-8"
            )
        )
        consumed = [
            json.loads((tools / name).read_text(encoding="utf-8"))
            for name in manifest["selection"]["consumed_manifests"]
        ]
        cases = manifest["cases"]
        consumed_ids = {
            case["id"] for source in consumed for case in source["cases"]
        }
        consumed_hosts = {
            case["host"] for source in consumed for case in source["cases"]
        }

        self.assertTrue(manifest["evaluated"])
        self.assertEqual(manifest["evaluation"]["decision"], "consumed-as-regression")
        self.assertEqual(manifest["evaluation"]["total"], 50)
        self.assertEqual(len(cases), 50)
        self.assertEqual(
            {case["dataset"] for case in cases}, {"dragnet", "scrapinghub"}
        )
        self.assertEqual(len({case["id"] for case in cases}), len(cases))
        self.assertEqual(len({case["host"] for case in cases}), len(cases))
        self.assertTrue({case["id"] for case in cases}.isdisjoint(consumed_ids))
        self.assertTrue({case["host"] for case in cases}.isdisjoint(consumed_hosts))

    def test_active_manifest_is_empty_and_blind(self):
        manifest = json.loads(
            Path(__file__).with_name("holdout_manifest.json").read_text(encoding="utf-8")
        )
        self.assertFalse(manifest["evaluated"])
        self.assertEqual(manifest["cases"], [])

    def test_selection_is_deterministic_and_excludes_consumed_cases(self):
        records = {
            "a": {"url": "https://one.example/a", "text": "Article A"},
            "b": {"url": "https://two.example/b", "text": "Article B"},
            "c": {"url": "https://three.example/c", "text": "Article C"},
        }
        first = select_cases(
            "test", records, 1, {"a"}, {"two.example"}, "text"
        )
        second = select_cases(
            "test", dict(reversed(records.items())), 1, {"a"}, {"two.example"}, "text"
        )

        self.assertEqual(first, second)
        self.assertEqual(first[0]["id"], "c")


if __name__ == "__main__":
    unittest.main()
