#!/usr/bin/env python3
"""
Batch save URLs to corpus with Readability.js expected output.

Usage:
    cd tools && uv run batch_corpus.py urls.txt
    cd tools && echo "https://example.com" | uv run batch_corpus.py -

Requirements:
    cd tools && pnpm install && uv sync
"""

import argparse
import hashlib
import subprocess
import sys
import time
import urllib.request
from pathlib import Path
from urllib.parse import urlparse


def sanitize_filename(url: str) -> str:
    """Generate a safe filename from URL."""
    parsed = urlparse(url)
    # Use domain + path, replacing unsafe chars
    name = f"{parsed.netloc}{parsed.path}"
    name = name.replace("/", "_").replace("?", "_").replace("&", "_")
    name = name.replace(".", "_").replace(":", "_")
    name = name.strip("_")
    # Truncate if too long, add hash for uniqueness
    if len(name) > 60:
        url_hash = hashlib.md5(url.encode()).hexdigest()[:8]
        name = name[:50] + "_" + url_hash
    return name


def fetch_url(url: str, user_agent: str) -> str:
    """Fetch HTML content from URL."""
    req = urllib.request.Request(
        url,
        headers={"User-Agent": user_agent},
    )
    with urllib.request.urlopen(req, timeout=30) as response:
        return response.read().decode("utf-8", errors="replace")


def run_readability_js(html_content: str) -> str | None:
    """Run Readability.js on HTML content and return extracted HTML."""
    tools_dir = Path(__file__).parent
    script = """
    const { Readability } = require('@mozilla/readability');
    const { JSDOM } = require('jsdom');

    let html = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', chunk => { html += chunk; });
    process.stdin.on('end', () => {
        const dom = new JSDOM(html);
        const reader = new Readability(dom.window.document);
        const article = reader.parse();

        if (article && article.content) {
            console.log(article.content);
        }
    });
    """

    try:
        result = subprocess.run(
            ["node", "-e", script],
            input=html_content,
            capture_output=True,
            text=True,
            timeout=30,
            cwd=tools_dir,
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError) as e:
        print(f"    Error running Readability.js: {e}")
    return None


def process_url(
    url: str,
    corpus_dir: Path,
    user_agent: str,
    overwrite: bool = False,
) -> bool:
    """Process a single URL and save to corpus."""
    filename = sanitize_filename(url)
    html_path = corpus_dir / f"{filename}.html"
    expected_path = corpus_dir / f"{filename}.expected.html"

    # Skip if already exists
    if html_path.exists() and expected_path.exists() and not overwrite:
        print(f"  Skipping (exists): {filename}")
        return True

    # Fetch HTML
    print(f"  Fetching: {url}")
    try:
        html = fetch_url(url, user_agent)
    except Exception as e:
        print(f"    Error fetching: {e}")
        return False

    # Run Readability.js
    print(f"    Running Readability.js...")
    expected = run_readability_js(html)
    if not expected:
        print(f"    Readability.js returned no content, skipping")
        return False

    # Save files
    html_path.write_text(html, encoding="utf-8")
    expected_path.write_text(expected, encoding="utf-8")
    print(f"    Saved: {filename} ({len(html):,} bytes, expected: {len(expected):,} bytes)")

    return True


def main():
    parser = argparse.ArgumentParser(
        description="Batch save URLs to corpus with Readability.js expected output"
    )
    parser.add_argument(
        "input",
        help="File with URLs (one per line) or - for stdin",
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
    parser.add_argument(
        "--delay",
        type=float,
        default=1.0,
        help="Delay between requests in seconds (default: 1.0)",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Overwrite existing files",
    )

    args = parser.parse_args()

    # Create corpus directory if needed
    args.corpus.mkdir(parents=True, exist_ok=True)

    # Read URLs
    if args.input == "-":
        urls = [line.strip() for line in sys.stdin if line.strip()]
    else:
        with open(args.input) as f:
            urls = [line.strip() for line in f if line.strip()]

    # Filter out comments and empty lines
    urls = [url for url in urls if url and not url.startswith("#")]

    print(f"Processing {len(urls)} URLs...")

    success_count = 0
    for i, url in enumerate(urls):
        print(f"[{i+1}/{len(urls)}] {url[:60]}...")
        if process_url(url, args.corpus, args.user_agent, args.overwrite):
            success_count += 1

        # Rate limiting
        if i < len(urls) - 1:
            time.sleep(args.delay)

    print(f"\nDone: {success_count}/{len(urls)} URLs processed successfully")


if __name__ == "__main__":
    main()
