import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from evaluate import compute_metrics, extract_text_from_html, load_case_filter, tokenize
from structure_metrics import compute_structure_metrics, extract_structure


class EvaluateTest(unittest.TestCase):
    def test_case_filter_ignores_comments_and_blank_lines(self):
        path = Path(self.id().replace(".", "_"))
        try:
            path.write_text("# fold\ncase-b\n\n case-a \ncase-b\n", encoding="utf-8")
            self.assertEqual(load_case_filter(path), {"case-a", "case-b"})
        finally:
            path.unlink(missing_ok=True)

    def test_extract_text_ignores_non_content_and_decodes_entities(self):
        html = "<style>hidden</style><p>A &amp; B</p><script>ignored()</script>"
        self.assertEqual(extract_text_from_html(html), "A & B")

    def test_tokenize_handles_unsegmented_scripts_as_characters(self):
        self.assertEqual(
            tokenize("Hello, 世界 ภาษา"),
            ["hello", "世", "界", "ภ", "า", "ษ", "า"],
        )

    def test_tokenize_handles_non_latin_words(self):
        self.assertEqual(
            tokenize("Γειά σου, мир! مرحبًا"),
            ["γειά", "σου", "мир", "مرحبًا"],
        )

    def test_metrics_penalize_missing_and_duplicate_content(self):
        metrics = compute_metrics("one two two three", "one two two two")
        self.assertAlmostEqual(metrics["precision"], 0.75)
        self.assertAlmostEqual(metrics["recall"], 0.75)
        self.assertAlmostEqual(metrics["f1"], 0.75)
        self.assertAlmostEqual(metrics["jaccard"], 0.6)

    def test_metrics_treat_two_empty_documents_as_equal(self):
        metrics = compute_metrics("", "")
        self.assertEqual(metrics, {"precision": 1.0, "recall": 1.0, "f1": 1.0, "jaccard": 1.0})

    def test_structure_metrics_compare_tags_and_resource_urls(self):
        expected = '<article><iframe src="https://example.com/video"></iframe></article>'
        actual = '<iframe src="https://example.com/video"></iframe>'
        self.assertEqual(extract_structure(expected), extract_structure(actual))
        self.assertEqual(
            compute_structure_metrics(expected, actual),
            {"precision": 1.0, "recall": 1.0, "f1": 1.0},
        )

    def test_structure_metrics_penalize_missing_resources(self):
        metrics = compute_structure_metrics(
            '<p><img src="one.jpg"><img src="two.jpg"></p>',
            '<p><img src="one.jpg"></p>',
        )
        self.assertAlmostEqual(metrics["precision"], 1.0)
        self.assertAlmostEqual(metrics["recall"], 0.5)
        self.assertAlmostEqual(metrics["f1"], 2 / 3)


if __name__ == "__main__":
    unittest.main()
