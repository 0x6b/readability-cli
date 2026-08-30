"""Language-aware text normalization and overlap metrics."""

import re
from collections import Counter
from html.parser import HTMLParser


TOKEN_RE = re.compile(
    r"[a-zA-Z0-9]+(?:['’][a-zA-Z0-9]+)*|"
    r"[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uac00-\ud7af]"
)


class TextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.ignored_depth = 0

    def handle_starttag(self, tag: str, attrs) -> None:
        if tag in {"script", "style", "template"}:
            self.ignored_depth += 1

    def handle_endtag(self, tag: str) -> None:
        if tag in {"script", "style", "template"} and self.ignored_depth:
            self.ignored_depth -= 1

    def handle_data(self, data: str) -> None:
        if not self.ignored_depth:
            self.parts.append(data)


def extract_text_from_html(html: str) -> str:
    parser = TextExtractor()
    parser.feed(html)
    return " ".join(" ".join(parser.parts).split())


def tokenize(text: str) -> list[str]:
    """Tokenize Latin text as words and CJK text as characters."""
    return TOKEN_RE.findall(text.casefold())


def compute_metrics(expected: str, actual: str) -> dict[str, float]:
    expected_tokens = Counter(tokenize(expected))
    actual_tokens = Counter(tokenize(actual))
    if not expected_tokens and not actual_tokens:
        return {"precision": 1.0, "recall": 1.0, "f1": 1.0, "jaccard": 1.0}
    overlap = sum((expected_tokens & actual_tokens).values())
    expected_count = sum(expected_tokens.values())
    actual_count = sum(actual_tokens.values())
    union = sum((expected_tokens | actual_tokens).values())
    precision = overlap / actual_count if actual_count else 0.0
    recall = overlap / expected_count if expected_count else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "jaccard": overlap / union if union else 0.0,
    }
