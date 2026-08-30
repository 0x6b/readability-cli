import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from import_scrapinghub_gold import expected_html, normalized_host, select_cases


class ImportScrapinghubGoldTest(unittest.TestCase):
    def test_committed_manifest_is_unconsumed_and_domain_diverse(self):
        tools = Path(__file__).parent
        manifest = json.loads(
            (tools / "scrapinghub_manifest.json").read_text(encoding="utf-8")
        )
        dragnet = json.loads(
            (tools / "dragnet_manifest.json").read_text(encoding="utf-8")
        )
        cases = manifest["cases"]
        dragnet_hosts = {case["host"] for case in dragnet["cases"]}

        self.assertFalse(manifest["evaluated"])
        self.assertEqual(len(cases), 50)
        self.assertEqual(len({case["id"] for case in cases}), 50)
        self.assertEqual(len({case["host"] for case in cases}), 50)
        self.assertTrue({case["host"] for case in cases}.isdisjoint(dragnet_hosts))

    def test_normalized_host_ignores_www_prefix(self):
        self.assertEqual(normalized_host("https://www.Example.com/article"), "example.com")

    def test_selection_is_deterministic_and_excludes_hosts(self):
        records = {
            "a": {"url": "https://one.example/a", "articleBody": "Article A"},
            "b": {"url": "https://www.one.example/b", "articleBody": "Article B"},
            "c": {"url": "https://two.example/c", "articleBody": "Article C"},
            "d": {"url": "https://three.example/d", "articleBody": "Article D"},
        }
        first = select_cases(records, 2, {"one.example"})
        second = select_cases(dict(reversed(records.items())), 2, {"one.example"})

        self.assertEqual(first, second)
        self.assertEqual(len({case["host"] for case in first}), 2)
        self.assertNotIn("one.example", {case["host"] for case in first})

    def test_expected_html_escapes_text_and_preserves_paragraphs(self):
        self.assertEqual(
            expected_html("First & foremost\n\nSecond < third"),
            "<article><p>First &amp; foremost</p><p>Second &lt; third</p></article>",
        )


if __name__ == "__main__":
    unittest.main()
