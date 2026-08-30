#!/usr/bin/env python3
"""Evaluate extraction quality against frozen expected HTML fixtures."""

import argparse
import json
import statistics
import subprocess
import sys
from pathlib import Path

from corpus_splits import SPLITS, in_split
from structure_metrics import compute_structure_metrics, extract_structure
from text_metrics import compute_metrics, extract_text_from_html, tokenize


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = REPO_ROOT / "tests" / "corpus"
DEFAULT_BINARY = REPO_ROOT / "target" / "release" / "rdbl"


def build_binary() -> None:
    subprocess.run(
        ["cargo", "build", "--release", "-p", "rdbl_cli"],
        cwd=REPO_ROOT,
        check=True,
    )


def run_extractor(binary: Path, html_path: Path, timeout: int) -> tuple[str, str]:
    try:
        result = subprocess.run(
            [str(binary), "--stdin", "--format", "html"],
            input=html_path.read_text(encoding="utf-8", errors="replace"),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f"timed out after {timeout}s") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RuntimeError(detail)
    return result.stdout, extract_text_from_html(result.stdout)


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate extraction quality")
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument(
        "--split",
        choices=(*SPLITS, "all"),
        default="test",
        help="Corpus split to evaluate (default: test)",
    )
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--timeout", type=int, default=30)
    parser.add_argument("--verbose", "-v", action="store_true")
    parser.add_argument("--json", action="store_true", help="Output results as JSON")
    args = parser.parse_args()

    if not args.no_build:
        build_binary()
    if not args.binary.is_file():
        parser.error(f"extractor binary not found: {args.binary}")

    results: list[dict] = []
    structure_results: list[dict] = []
    failures: list[dict[str, str]] = []
    missing_expected: list[str] = []
    non_text_cases: list[str] = []
    structural_failures: list[dict] = []

    for html_file in sorted(args.corpus.glob("*.html")):
        if html_file.stem.endswith(".expected") or not in_split(html_file.stem, args.split):
            continue
        expected_file = args.corpus / f"{html_file.stem}.expected.html"
        if not expected_file.exists():
            missing_expected.append(html_file.stem)
            continue

        expected_html = expected_file.read_text(encoding="utf-8", errors="replace")
        expected_text = extract_text_from_html(expected_html)
        try:
            actual_html, actual_text = run_extractor(args.binary, html_file, args.timeout)
        except RuntimeError as error:
            failures.append({"case": html_file.stem, "error": str(error)})
            if args.verbose:
                print(f"ERROR {html_file.stem}: {error}", file=sys.stderr)
            continue

        if expected_structure := extract_structure(expected_html):
            structure_metrics = compute_structure_metrics(expected_html, actual_html)
            structure_result = {
                "case": html_file.stem,
                **structure_metrics,
                "expected_tokens": sum(expected_structure.values()),
                "actual_tokens": sum(extract_structure(actual_html).values()),
            }
            structure_results.append(structure_result)
        else:
            structure_result = None

        if not tokenize(expected_text):
            non_text_cases.append(html_file.stem)
            if structure_result is None or structure_result["f1"] < 0.99:
                structural_failures.append(
                    structure_result or {"case": html_file.stem, "f1": 0.0, "reason": "no expected structure"}
                )
            continue

        metrics = compute_metrics(expected_text, actual_text)
        result = {
            "case": html_file.stem,
            **metrics,
            "expected_tokens": len(tokenize(expected_text)),
            "actual_tokens": len(tokenize(actual_text)),
        }
        results.append(result)
        if args.verbose:
            print(
                f"{html_file.stem}: F1={metrics['f1']:.1%} "
                f"P={metrics['precision']:.1%} R={metrics['recall']:.1%}"
            )

    summary = {
        "split": args.split,
        "total": len(results),
        "failures": failures,
        "missing_expected": missing_expected,
        "non_text_cases": non_text_cases,
        "structural_failures": structural_failures,
        "average_precision": statistics.fmean(r["precision"] for r in results) if results else 0,
        "average_recall": statistics.fmean(r["recall"] for r in results) if results else 0,
        "average_f1": statistics.fmean(r["f1"] for r in results) if results else 0,
        "median_f1": statistics.median(r["f1"] for r in results) if results else 0,
        "average_jaccard": statistics.fmean(r["jaccard"] for r in results) if results else 0,
        "high": sum(r["f1"] >= 0.7 for r in results),
        "medium": sum(0.5 <= r["f1"] < 0.7 for r in results),
        "low": sum(r["f1"] < 0.5 for r in results),
        "worst_cases": sorted(results, key=lambda r: r["f1"])[:10],
        "structure_total": len(structure_results),
        "average_structure_precision": (
            statistics.fmean(r["precision"] for r in structure_results) if structure_results else 0
        ),
        "average_structure_recall": (
            statistics.fmean(r["recall"] for r in structure_results) if structure_results else 0
        ),
        "average_structure_f1": (
            statistics.fmean(r["f1"] for r in structure_results) if structure_results else 0
        ),
        "worst_structure_cases": sorted(structure_results, key=lambda r: r["f1"])[:10],
    }

    if args.json:
        print(json.dumps(summary, ensure_ascii=False))
    else:
        print(f"\nEvaluated {len(results)} {args.split} cases")
        print(
            f"Macro average: F1={summary['average_f1']:.1%}, "
            f"precision={summary['average_precision']:.1%}, "
            f"recall={summary['average_recall']:.1%}"
        )
        print(f"Median F1: {summary['median_f1']:.1%}")
        print(
            f"F1 buckets: ≥70% {summary['high']}, "
            f"50–70% {summary['medium']}, <50% {summary['low']}"
        )
        if missing_expected:
            print(f"Missing expected output: {', '.join(missing_expected)}")
        if non_text_cases:
            print(f"Scored by structure only: {', '.join(non_text_cases)}")
        print(
            f"Structural macro average ({len(structure_results)} cases): "
            f"F1={summary['average_structure_f1']:.1%}, "
            f"precision={summary['average_structure_precision']:.1%}, "
            f"recall={summary['average_structure_recall']:.1%}"
        )
        print("\nLowest F1 cases:")
        for result in summary["worst_cases"]:
            print(
                f"  {result['case']}: F1={result['f1']:.1%} "
                f"P={result['precision']:.1%} R={result['recall']:.1%}"
            )

    return 1 if failures or missing_expected or structural_failures or not results else 0


if __name__ == "__main__":
    raise SystemExit(main())
