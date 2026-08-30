"""Structural HTML agreement metrics for non-text and rich-content fixtures."""

from collections import Counter
from html.parser import HTMLParser


STRUCTURAL_TAGS = {
    "a",
    "audio",
    "blockquote",
    "code",
    "iframe",
    "img",
    "ol",
    "pre",
    "table",
    "ul",
    "video",
}
URL_ATTRIBUTES = {"a": "href", "audio": "src", "iframe": "src", "img": "src", "video": "src"}


class StructureExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.tokens: Counter[str] = Counter()

    def handle_starttag(self, tag: str, attrs) -> None:
        if tag not in STRUCTURAL_TAGS:
            return
        self.tokens[f"tag:{tag}"] += 1
        url_attribute = URL_ATTRIBUTES.get(tag)
        if url_attribute:
            values = dict(attrs)
            if value := values.get(url_attribute):
                self.tokens[f"{tag}:{url_attribute}:{value.strip()}"] += 1

    handle_startendtag = handle_starttag


def extract_structure(html: str) -> Counter[str]:
    parser = StructureExtractor()
    parser.feed(html)
    return parser.tokens


def compute_structure_metrics(expected_html: str, actual_html: str) -> dict[str, float]:
    expected = extract_structure(expected_html)
    actual = extract_structure(actual_html)
    if not expected and not actual:
        return {"precision": 1.0, "recall": 1.0, "f1": 1.0}
    overlap = sum((expected & actual).values())
    expected_count = sum(expected.values())
    actual_count = sum(actual.values())
    precision = overlap / actual_count if actual_count else 0.0
    recall = overlap / expected_count if expected_count else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {"precision": precision, "recall": recall, "f1": f1}
