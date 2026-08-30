import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from import_dragnet_gold import expected_html, normalized_host, select_cases


class ImportDragnetGoldTest(unittest.TestCase):
    def test_committed_manifest_records_domain_diverse_evaluation(self):
        manifest = json.loads(
            Path(__file__).with_name("dragnet_manifest.json").read_text(encoding="utf-8")
        )
        cases = manifest["cases"]
        self.assertTrue(manifest["evaluated"])
        self.assertEqual(manifest["evaluation"]["total"], 50)
        self.assertEqual(manifest["evaluation"]["failures"], 0)
        self.assertEqual(len(cases), 50)
        self.assertEqual(len({case["id"] for case in cases}), 50)
        self.assertEqual(len({case["host"] for case in cases}), 50)

    def test_normalized_host_ignores_www_prefix(self):
        self.assertEqual(normalized_host("https://www.Example.com/article"), "example.com")

    def test_selection_is_deterministic_and_uses_unique_hosts(self):
        entries = {
            "a": {"url": "https://one.example/a"},
            "b": {"url": "https://www.one.example/b"},
            "c": {"url": "https://two.example/c"},
        }
        first = select_cases(entries, 2)
        second = select_cases(dict(reversed(entries.items())), 2)
        self.assertEqual(first, second)
        self.assertEqual(len({case["host"] for case in first}), 2)

    def test_expected_html_escapes_text_and_preserves_paragraphs(self):
        self.assertEqual(
            expected_html("First & foremost\n\nSecond < third"),
            "<article><p>First &amp; foremost</p><p>Second &lt; third</p></article>",
        )


if __name__ == "__main__":
    unittest.main()
