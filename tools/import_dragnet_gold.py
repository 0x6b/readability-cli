#!/usr/bin/env python3
"""Import a fixed, domain-diverse sample of manually annotated Dragnet data."""

import argparse
import hashlib
import html
import json
import tarfile
import tempfile
import tomllib
import urllib.request
from pathlib import Path
from urllib.parse import urlparse


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = REPO_ROOT / "tests" / "external" / "dragnet"
DEFAULT_MANIFEST = Path(__file__).with_name("dragnet_manifest.json")
DEFAULT_CACHE = Path.home() / ".cache" / "readability-cli" / "dragnet"
UPSTREAM_REPOSITORY = "https://github.com/dragnet-org/dragnet_data"
UPSTREAM_COMMIT = "5d97da4c3c5cf775fa02794b84d31786b8d51d3a"
SELECTION_SEED = "readability-cli-dragnet-v1"
ARCHIVES = {
    "html": {
        "url": f"https://media.githubusercontent.com/media/dragnet-org/dragnet_data/{UPSTREAM_COMMIT}/data/html.tar.gz",
        "sha256": "eb80881cef7cbc7ad0086d0c2ac9aacf95d27f223fa66821457bad4b39ff39b7",
    },
    "meta": {
        "url": f"https://media.githubusercontent.com/media/dragnet-org/dragnet_data/{UPSTREAM_COMMIT}/data/meta.tar.gz",
        "sha256": "5e971b3ba9616876a1a648727d40283b2768c23531a14aca9b6451aa5e6c16d2",
    },
}


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download_archive(name: str, cache: Path) -> Path:
    archive = ARCHIVES[name]
    destination = cache / f"{name}.tar.gz"
    if destination.is_file() and file_sha256(destination) == archive["sha256"]:
        return destination

    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temporary:
        temporary_path = Path(temporary.name)
    try:
        print(f"Downloading {name} archive")
        urllib.request.urlretrieve(archive["url"], temporary_path)
        actual = file_sha256(temporary_path)
        if actual != archive["sha256"]:
            raise ValueError(f"{name} archive checksum mismatch: {actual}")
        temporary_path.replace(destination)
    finally:
        temporary_path.unlink(missing_ok=True)
    return destination


def read_archive(path: Path, suffix: str) -> dict[str, bytes]:
    files = {}
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            member_path = Path(member.name)
            if not member.isfile() or member_path.suffix != suffix:
                continue
            source = archive.extractfile(member)
            if source is not None:
                files[member_path.stem] = source.read()
    return files


def normalized_host(url: str) -> str:
    host = (urlparse(url).hostname or "").lower()
    return host.removeprefix("www.")


def load_metadata(path: Path) -> dict[str, dict]:
    entries = {}
    for case_id, content in read_archive(path, ".toml").items():
        entry = tomllib.loads(content.decode("utf-8"))
        if entry.get("text", "").strip() and normalized_host(entry.get("url", "")):
            entries[case_id] = entry
    return entries


def select_cases(entries: dict[str, dict], count: int) -> list[dict[str, str]]:
    ranked = sorted(
        entries.items(),
        key=lambda item: hashlib.sha256(
            f"{SELECTION_SEED}\0{item[0]}".encode()
        ).digest(),
    )
    selected = []
    seen_hosts = set()
    for case_id, entry in ranked:
        host = normalized_host(entry["url"])
        if host in seen_hosts:
            continue
        seen_hosts.add(host)
        selected.append(
            {
                "case": f"dragnet_{case_id}",
                "id": case_id,
                "url": entry["url"],
                "host": host,
            }
        )
        if len(selected) == count:
            return selected
    raise ValueError(f"requested {count} cases but only found {len(selected)} unique hosts")


def make_manifest(entries: dict[str, dict], count: int) -> dict:
    return {
        "dataset": "Dragnet gold data",
        "upstream_repository": UPSTREAM_REPOSITORY,
        "upstream_commit": UPSTREAM_COMMIT,
        "license": "CC-BY-4.0",
        "annotation": "manual main-text extraction",
        "evaluated": False,
        "selection": {
            "seed": SELECTION_SEED,
            "count": count,
            "one_case_per_normalized_host": True,
        },
        "archives": ARCHIVES,
        "cases": select_cases(entries, count),
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
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--count", type=int, default=50)
    parser.add_argument("--write-manifest", action="store_true")
    args = parser.parse_args()

    meta_archive = download_archive("meta", args.cache)
    metadata = load_metadata(meta_archive)
    if args.write_manifest:
        if args.manifest.exists():
            parser.error(f"manifest already exists: {args.manifest}")
        manifest = make_manifest(metadata, args.count)
        args.manifest.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        print(f"Wrote manifest with {len(manifest['cases'])} cases: {args.manifest}")
    else:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))

    html_archive = download_archive("html", args.cache)
    source_html = read_archive(html_archive, ".html")
    args.corpus.mkdir(parents=True, exist_ok=True)
    for case in manifest["cases"]:
        case_id = case["id"]
        if case_id not in source_html or case_id not in metadata:
            raise ValueError(f"missing upstream pair: {case_id}")
        (args.corpus / f"{case['case']}.html").write_bytes(source_html[case_id])
        (args.corpus / f"{case['case']}.expected.html").write_text(
            expected_html(metadata[case_id]["text"]),
            encoding="utf-8",
        )

    print(f"Imported {len(manifest['cases'])} cases to {args.corpus}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
