#!/usr/bin/env python3
"""
Save a URL as an HTML file in the corpus directory.

Usage:
    cd tools && uv run save_corpus.py https://example.com/article
    cd tools && uv run save_corpus.py https://example.com/article --name my_article

Requirements:
    cd tools && uv sync
"""

import argparse
import hashlib
import sys
import urllib.request
from pathlib import Path
from urllib.parse import urlparse


def sanitize_filename(url: str) -> str:
    """Generate a safe filename from URL."""
    parsed = urlparse(url)
    # Use domain + path, replacing unsafe chars
    name = f"{parsed.netloc}{parsed.path}"
    name = name.replace("/", "_").replace("?", "_").replace("&", "_")
    name = name.strip("_")
    # Truncate if too long, add hash for uniqueness
    if len(name) > 80:
        url_hash = hashlib.md5(url.encode()).hexdigest()[:8]
        name = name[:70] + "_" + url_hash
    return name + ".html"


def fetch_url(url: str, user_agent: str) -> str:
    """Fetch HTML content from URL."""
    req = urllib.request.Request(
        url,
        headers={"User-Agent": user_agent},
    )
    with urllib.request.urlopen(req, timeout=30) as response:
        return response.read().decode("utf-8", errors="replace")


def main():
    parser = argparse.ArgumentParser(
        description="Save a URL as an HTML file in the corpus directory"
    )
    parser.add_argument("url", help="URL to fetch and save")
    parser.add_argument(
        "--name",
        type=str,
        default=None,
        help="Custom filename (without .html extension)",
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path(__file__).parent.parent / "tests" / "corpus",
        help="Corpus directory (default: tests/corpus)",
    )
    parser.add_argument(
        "--user-agent",
        type=str,
        default="Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:139.0) Gecko/20100101 Firefox/139.0",
        help="User agent string",
    )

    args = parser.parse_args()

    # Create corpus directory if needed
    args.corpus.mkdir(parents=True, exist_ok=True)

    # Generate filename
    if args.name:
        filename = args.name + ".html"
    else:
        filename = sanitize_filename(args.url)

    output_path = args.corpus / filename

    # Check if file already exists
    if output_path.exists():
        print(f"File already exists: {output_path}")
        response = input("Overwrite? [y/N] ").strip().lower()
        if response != "y":
            print("Aborted.")
            sys.exit(0)

    # Fetch and save
    print(f"Fetching: {args.url}")
    try:
        html = fetch_url(args.url, args.user_agent)
    except Exception as e:
        print(f"Error fetching URL: {e}")
        sys.exit(1)

    output_path.write_text(html, encoding="utf-8")
    print(f"Saved: {output_path} ({len(html):,} bytes)")


if __name__ == "__main__":
    main()
