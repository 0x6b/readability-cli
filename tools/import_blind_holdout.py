#!/usr/bin/env python3
"""Import a frozen blind cohort from unused human-annotated gold cases."""

import argparse
import hashlib
import json
from pathlib import Path

import import_dragnet_gold as dragnet
import import_scrapinghub_gold as scrapinghub


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = REPO_ROOT / "tests" / "external" / "holdout"
DEFAULT_MANIFEST = Path(__file__).with_name("holdout_manifest.json")
DEFAULT_CONSUMED_MANIFESTS = (
    Path(__file__).with_name("dragnet_manifest.json"),
    Path(__file__).with_name("scrapinghub_manifest.json"),
)
SELECTION_SEED = "readability-cli-blind-holdout-v1"
DEFAULT_CASES_PER_DATASET = 25


def load_consumed_cases(paths: tuple[Path, ...]) -> tuple[set[str], set[str]]:
    case_ids = set()
    hosts = set()
    for path in paths:
        manifest = json.loads(path.read_text(encoding="utf-8"))
        case_ids.update(case["id"] for case in manifest["cases"])
        hosts.update(case["host"] for case in manifest["cases"])
    return case_ids, hosts


def select_cases(
    dataset: str,
    records: dict[str, dict],
    count: int,
    excluded_ids: set[str],
    excluded_hosts: set[str],
    text_field: str,
) -> list[dict[str, str]]:
    ranked = sorted(
        records.items(),
        key=lambda item: hashlib.sha256(
            f"{SELECTION_SEED}\0{dataset}\0{item[0]}".encode()
        ).digest(),
    )
    selected = []
    seen_hosts = set(excluded_hosts)
    for case_id, record in ranked:
        host = dragnet.normalized_host(record.get("url", ""))
        if (
            case_id in excluded_ids
            or not record.get(text_field, "").strip()
            or not host
            or host in seen_hosts
        ):
            continue
        seen_hosts.add(host)
        selected.append(
            {
                "case": f"{dataset}_{case_id}",
                "dataset": dataset,
                "id": case_id,
                "url": record["url"],
                "host": host,
            }
        )
        if len(selected) == count:
            return selected
    raise ValueError(f"requested {count} {dataset} cases but found {len(selected)}")


def make_manifest(
    dragnet_records: dict[str, dict],
    scrapinghub_records: dict[str, dict],
    count: int,
    consumed_manifests: tuple[Path, ...],
) -> dict:
    consumed_ids, consumed_hosts = load_consumed_cases(consumed_manifests)
    dragnet_cases = select_cases(
        "dragnet",
        dragnet_records,
        count,
        consumed_ids,
        consumed_hosts,
        "text",
    )
    selected_hosts = consumed_hosts | {case["host"] for case in dragnet_cases}
    scrapinghub_cases = select_cases(
        "scrapinghub",
        scrapinghub_records,
        count,
        consumed_ids,
        selected_hosts,
        "articleBody",
    )
    return {
        "created": "2026-08-31",
        "teacher": "human-annotated main article text",
        "evaluated": False,
        "selection": {
            "seed": SELECTION_SEED,
            "cases_per_dataset": count,
            "one_case_per_normalized_host": True,
            "excludes_consumed_case_ids_and_hosts": True,
            "consumed_manifests": [path.name for path in consumed_manifests],
        },
        "sources": {
            "dragnet": {
                "repository": dragnet.UPSTREAM_REPOSITORY,
                "commit": dragnet.UPSTREAM_COMMIT,
                "license": "CC-BY-4.0",
            },
            "scrapinghub": {
                "repository": scrapinghub.UPSTREAM_REPOSITORY,
                "commit": scrapinghub.UPSTREAM_COMMIT,
                "license": "MIT (publisher content may retain third-party rights)",
            },
        },
        "cases": dragnet_cases + scrapinghub_cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--dragnet-cache", type=Path, default=dragnet.DEFAULT_CACHE)
    parser.add_argument(
        "--scrapinghub-cache", type=Path, default=scrapinghub.DEFAULT_CACHE
    )
    parser.add_argument("--cases-per-dataset", type=int, default=DEFAULT_CASES_PER_DATASET)
    parser.add_argument("--write-manifest", action="store_true")
    args = parser.parse_args()

    dragnet_meta_archive = dragnet.download_archive("meta", args.dragnet_cache)
    dragnet_records = dragnet.load_metadata(dragnet_meta_archive)
    scrapinghub_archive = scrapinghub.download_archive(args.scrapinghub_cache)
    scrapinghub_records, scrapinghub_html = scrapinghub.load_archive(scrapinghub_archive)

    if args.write_manifest:
        if args.manifest.exists():
            parser.error(f"manifest already exists: {args.manifest}")
        manifest = make_manifest(
            dragnet_records,
            scrapinghub_records,
            args.cases_per_dataset,
            DEFAULT_CONSUMED_MANIFESTS,
        )
        args.manifest.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        print(f"Froze {len(manifest['cases'])} blind cases: {args.manifest}")
    else:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))

    dragnet_html = dragnet.read_archive(
        dragnet.download_archive("html", args.dragnet_cache), ".html"
    )
    args.corpus.mkdir(parents=True, exist_ok=True)
    for case in manifest["cases"]:
        case_id = case["id"]
        if case["dataset"] == "dragnet":
            source = dragnet_html[case_id]
            expected = dragnet.expected_html(dragnet_records[case_id]["text"])
        else:
            source = scrapinghub_html[case_id]
            expected = scrapinghub.expected_html(
                scrapinghub_records[case_id]["articleBody"]
            )
        (args.corpus / f"{case['case']}.html").write_bytes(source)
        (args.corpus / f"{case['case']}.expected.html").write_text(
            expected, encoding="utf-8"
        )

    print(f"Imported {len(manifest['cases'])} cases to {args.corpus}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
