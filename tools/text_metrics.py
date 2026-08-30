"""Language-aware text normalization and overlap metrics."""

import unicodedata
from collections import Counter
from html.parser import HTMLParser


def is_character_token(character: str) -> bool:
    codepoint = ord(character)
    return (
        0x3040 <= codepoint <= 0x30FF
        or 0x3400 <= codepoint <= 0x4DBF
        or 0x4E00 <= codepoint <= 0x9FFF
        or 0xF900 <= codepoint <= 0xFAFF
        or 0xAC00 <= codepoint <= 0xD7AF
        or 0x0E00 <= codepoint <= 0x0EFF  # Thai and Lao
        or 0x1000 <= codepoint <= 0x109F  # Myanmar
        or 0x1780 <= codepoint <= 0x17FF  # Khmer
    )


def is_word_character(character: str) -> bool:
    return unicodedata.category(character)[0] in {"L", "M", "N"}


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
    """Tokenize Unicode text as words and unsegmented scripts as characters."""
    text = text.casefold()
    tokens: list[str] = []
    word: list[str] = []

    def finish_word() -> None:
        if word:
            tokens.append("".join(word))
            word.clear()

    for index, character in enumerate(text):
        if is_character_token(character):
            finish_word()
            tokens.append(character)
        elif is_word_character(character):
            word.append(character)
        elif (
            character in {"'", "’"}
            and word
            and index + 1 < len(text)
            and is_word_character(text[index + 1])
            and not is_character_token(text[index + 1])
        ):
            word.append(character)
        else:
            finish_word()

    finish_word()
    return tokens


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
