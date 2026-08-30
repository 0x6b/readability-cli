#!/usr/bin/env python3
"""Import a fixed, host-diverse sample of Scrapinghub's human gold data."""

import argparse
import gzip
import hashlib
import html
import json
import tarfile
import tempfile
import urllib.request
from pathlib import Path
from urllib.parse import urlparse


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = REPO_ROOT / "tests" / "external" / "scrapinghub"
DEFAULT_MANIFEST = Path(__file__).with_name("scrapinghub_manifest.json")
DEFAULT_EXCLUDED_MANIFEST = Path(__file__).with_name("dragnet_manifest.json")
DEFAULT_CACHE = Path.home() / ".cache" / "readability-cli" / "scrapinghub"
UPSTREAM_REPOSITORY = "https://github.com/scrapinghub/article-extraction-benchmark"
UPSTREAM_COMMIT = "11b407d43934becc20873bb3dcd5e3d2591746d3"
SELECTION_SEED = "readability-cli-scrapinghub-v1"
ARCHIVE = {
    "url": f"{UPSTREAM_REPOSITORY}/archive/{UPSTREAM_COMMIT}.tar.gz",
    "sha256": "c1d8f4b058f94b7c7c39f0968794c777a47128b58709c2ff58eec42f4487c7d0",
}
UPSTREAM_CASE_COUNT = 181


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download_archive(cache: Path) -> Path:
    destination = cache / "article-extraction-benchmark.tar.gz"
    if destination.is_file() and file_sha256(destination) == ARCHIVE["sha256"]:
        return destination

    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temporary:
        temporary_path = Path(temporary.name)
    try:
        print("Downloading Scrapinghub article extraction benchmark")
        urllib.request.urlretrieve(ARCHIVE["url"], temporary_path)
        actual = file_sha256(temporary_path)
        if actual != ARCHIVE["sha256"]:
            raise ValueError(f"archive checksum mismatch: {actual}")
        temporary_path.replace(destination)
    finally:
        temporary_path.unlink(missing_ok=True)
    return destination


def load_archive(path: Path) -> tuple[dict[str, dict], dict[str, bytes]]:
    records = None
    source_html = {}
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            if not member.isfile():
                continue
            member_path = Path(member.name)
            source = archive.extractfile(member)
            if source is None:
                continue
            if member_path.name == "ground-truth.json":
                records = json.load(source)
            elif member_path.parent.name == "html" and member_path.name.endswith(".html.gz"):
                case_id = member_path.name.removesuffix(".html.gz")
                source_html[case_id] = gzip.decompress(source.read())

    if records is None:
        raise ValueError("ground-truth.json is missing from archive")
    if len(records) != UPSTREAM_CASE_COUNT or len(source_html) != UPSTREAM_CASE_COUNT:
        raise ValueError(
            f"expected {UPSTREAM_CASE_COUNT} gold/HTML pairs, found "
            f"{len(records)} gold and {len(source_html)} HTML"
        )
    if records.keys() != source_html.keys():
        raise ValueError("ground-truth and HTML case IDs do not match")
    return records, source_html


def normalized_host(url: str) -> str:
    host = (urlparse(url).hostname or "").lower()
    return host.removeprefix("www.")


def load_excluded_hosts(path: Path) -> set[str]:
    manifest = json.loads(path.read_text(encoding="utf-8"))
    return {case["host"] for case in manifest["cases"]}


def select_cases(
    records: dict[str, dict], count: int, excluded_hosts: set[str]
) -> list[dict[str, str]]:
    ranked = sorted(
        records.items(),
        key=lambda item: hashlib.sha256(
            f"{SELECTION_SEED}\0{item[0]}".encode()
        ).digest(),
    )
    selected = []
    seen_hosts = set(excluded_hosts)
    for case_id, record in ranked:
        host = normalized_host(record.get("url", ""))
        if not record.get("articleBody", "").strip() or not host or host in seen_hosts:
            continue
        seen_hosts.add(host)
        selected.append(
            {
                "case": f"scrapinghub_{case_id}",
                "id": case_id,
                "url": record["url"],
                "host": host,
            }
        )
        if len(selected) == count:
            return selected
    raise ValueError(f"requested {count} cases but only found {len(selected)} eligible hosts")


def make_manifest(
    records: dict[str, dict], count: int, excluded_hosts: set[str]
) -> dict:
    return {
        "dataset": "Scrapinghub Article Extraction Benchmark",
        "upstream_repository": UPSTREAM_REPOSITORY,
        "upstream_commit": UPSTREAM_COMMIT,
        "license": "MIT (publisher content may retain third-party rights)",
        "annotation": "human-labeled articleBody text",
        "evaluated": False,
        "selection": {
            "seed": SELECTION_SEED,
            "count": count,
            "one_case_per_normalized_host": True,
            "excludes_dragnet_hosts": True,
        },
        "archive": ARCHIVE,
        "cases": select_cases(records, count, excluded_hosts),
    }


def expected_html(text: str) -> str:
    paragraphs = [paragraph.strip() for paragraph in text.split("\n\n") if paragraph.strip()]
    return "<article>" + "".join(
        f"<p>{html.escape(paragraph)}</p>" for paragraph in paragraphs
    ) + "</article>"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--excluded-manifest", type=Path, default=DEFAULT_EXCLUDED_MANIFEST)
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--count", type=int, default=50)
    parser.add_argument("--write-manifest", action="store_true")
    args = parser.parse_args()

    archive = download_archive(args.cache)
    records, source_html = load_archive(archive)
    if args.write_manifest:
        if args.manifest.exists():
            parser.error(f"manifest already exists: {args.manifest}")
        manifest = make_manifest(
            records, args.count, load_excluded_hosts(args.excluded_manifest)
        )
        args.manifest.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        print(f"Wrote manifest with {len(manifest['cases'])} cases: {args.manifest}")
    else:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))

    args.corpus.mkdir(parents=True, exist_ok=True)
    for case in manifest["cases"]:
        case_id = case["id"]
        if case_id not in source_html or case_id not in records:
            raise ValueError(f"missing upstream pair: {case_id}")
        (args.corpus / f"{case['case']}.html").write_bytes(source_html[case_id])
        (args.corpus / f"{case['case']}.expected.html").write_text(
            expected_html(records[case_id]["articleBody"]),
            encoding="utf-8",
        )

    print(f"Imported {len(manifest['cases'])} cases to {args.corpus}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
