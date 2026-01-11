#!/usr/bin/env python3
"""
Evaluate extraction accuracy against Mozilla Readability expected outputs.

Usage:
    cd tools && uv run evaluate.py --corpus ../tests/corpus
"""

import argparse
import re
import subprocess
from pathlib import Path


def extract_text_from_html(html: str) -> str:
    """Extract plain text from HTML."""
    html = re.sub(r"<(script|style)[^>]*>.*?</\1>", "", html, flags=re.DOTALL | re.IGNORECASE)
    text = re.sub(r"<[^>]+>", " ", html)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def compute_jaccard(text1: str, text2: str) -> float:
    """Compute Jaccard similarity between two texts."""
    words1 = set(text1.lower().split())
    words2 = set(text2.lower().split())
    if not words1 or not words2:
        return 0.0
    intersection = len(words1 & words2)
    union = len(words1 | words2)
    return intersection / union if union > 0 else 0.0


def run_extractor(html_path: Path) -> str:
    """Run our extractor on an HTML file."""
    result = subprocess.run(
        ["cargo", "run", "--release", "--", "--stdin", "--format", "text"],
        input=html_path.read_text(errors="replace"),
        capture_output=True,
        text=True,
        timeout=30,
        cwd=html_path.parent.parent.parent,
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def main():
    parser = argparse.ArgumentParser(description="Evaluate extraction accuracy")
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path(__file__).parent.parent / "tests" / "corpus",
    )
    parser.add_argument("--verbose", "-v", action="store_true")
    args = parser.parse_args()

    results = []

    for html_file in sorted(args.corpus.glob("*.html")):
        if html_file.stem.endswith(".expected"):
            continue

        expected_file = args.corpus / f"{html_file.stem}.expected.html"
        if not expected_file.exists():
            continue

        # Get expected text
        expected_html = expected_file.read_text(errors="replace")
        expected_text = extract_text_from_html(expected_html)

        # Get our extraction
        our_text = run_extractor(html_file)

        # Compute similarity
        similarity = compute_jaccard(expected_text, our_text)
        results.append((html_file.stem, similarity, len(expected_text.split()), len(our_text.split())))

        if args.verbose:
            status = "✓" if similarity >= 0.5 else "✗"
            print(f"{status} {html_file.stem}: {similarity:.2%}")

    # Summary
    print(f"\n{'='*60}")
    print(f"Evaluated {len(results)} test cases")

    similarities = [r[1] for r in results]
    avg = sum(similarities) / len(similarities) if similarities else 0

    high = sum(1 for s in similarities if s >= 0.7)
    medium = sum(1 for s in similarities if 0.5 <= s < 0.7)
    low = sum(1 for s in similarities if s < 0.5)

    print(f"\nJaccard similarity:")
    print(f"  Average: {avg:.1%}")
    print(f"  High (≥70%): {high} ({high/len(results)*100:.0f}%)")
    print(f"  Medium (50-70%): {medium} ({medium/len(results)*100:.0f}%)")
    print(f"  Low (<50%): {low} ({low/len(results)*100:.0f}%)")

    # Worst cases
    print(f"\nLowest similarity cases:")
    for name, sim, exp_words, our_words in sorted(results, key=lambda x: x[1])[:10]:
        print(f"  {name}: {sim:.1%} (expected {exp_words} words, got {our_words})")


if __name__ == "__main__":
    main()
