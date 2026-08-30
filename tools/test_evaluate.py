import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from evaluate import compute_metrics, extract_text_from_html, tokenize


class EvaluateTest(unittest.TestCase):
    def test_extract_text_ignores_non_content_and_decodes_entities(self):
        html = "<style>hidden</style><p>A &amp; B</p><script>ignored()</script>"
        self.assertEqual(extract_text_from_html(html), "A & B")

    def test_tokenize_handles_cjk_without_spaces(self):
        self.assertEqual(tokenize("Hello, 世界。"), ["hello", "世", "界"])

    def test_metrics_penalize_missing_and_duplicate_content(self):
        metrics = compute_metrics("one two two three", "one two two two")
        self.assertAlmostEqual(metrics["precision"], 0.75)
        self.assertAlmostEqual(metrics["recall"], 0.75)
        self.assertAlmostEqual(metrics["f1"], 0.75)
        self.assertAlmostEqual(metrics["jaccard"], 0.6)

    def test_metrics_treat_two_empty_documents_as_equal(self):
        metrics = compute_metrics("", "")
        self.assertEqual(metrics, {"precision": 1.0, "recall": 1.0, "f1": 1.0, "jaccard": 1.0})


if __name__ == "__main__":
    unittest.main()
