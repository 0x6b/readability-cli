#!/usr/bin/env python3
"""
Import Mozilla Readability test cases into the corpus.

Usage:
    cd tools && uv run import_mozilla_tests.py /path/to/readability/test/test-pages
"""

import argparse
import shutil
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(
        description="Import Mozilla Readability test cases into corpus"
    )
    parser.add_argument(
        "test_pages_dir",
        type=Path,
        help="Path to readability/test/test-pages directory",
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path(__file__).parent.parent / "tests" / "corpus",
        help="Corpus directory (default: tests/corpus)",
    )

    args = parser.parse_args()

    if not args.test_pages_dir.exists():
        print(f"Error: {args.test_pages_dir} does not exist")
        return 1

    args.corpus.mkdir(parents=True, exist_ok=True)

    imported = 0
    skipped = 0

    for test_dir in sorted(args.test_pages_dir.iterdir()):
        if not test_dir.is_dir():
            continue

        source = test_dir / "source.html"
        expected = test_dir / "expected.html"

        if not source.exists():
            print(f"Skip {test_dir.name}: no source.html")
            skipped += 1
            continue

        # Copy source.html as {name}.html
        dest_source = args.corpus / f"{test_dir.name}.html"
        shutil.copy(source, dest_source)

        # Copy expected.html as {name}.expected.html
        if expected.exists():
            dest_expected = args.corpus / f"{test_dir.name}.expected.html"
            shutil.copy(expected, dest_expected)
            print(f"Imported: {test_dir.name} (with expected)")
        else:
            print(f"Imported: {test_dir.name} (source only)")

        imported += 1

    print(f"\nImported {imported} test cases, skipped {skipped}")


if __name__ == "__main__":
    main()
