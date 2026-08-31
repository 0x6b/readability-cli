#!/usr/bin/env python3
"""Compare pairwise ranking with the embedded model using grouped folds."""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

from model_selection import assign_group_folds, summarize_fold_metrics
from train_logreg import (
    combine_page_data,
    combine_pairwise_page_data,
    generate_classification_page_data,
    generate_pairwise_page_data,
    load_corpus,
    save_weights,
    train_model,
    train_pairwise_ranker,
)


REPO_ROOT = Path(__file__).resolve().parent.parent
TOOLS_DIR = Path(__file__).resolve().parent
MODEL_PATH = REPO_ROOT / "crates" / "rdbl_core" / "src" / "model.rs"


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(command, cwd=REPO_ROOT, check=True, **kwargs)


def evaluate_cases(corpus: Path, cases: list[str], directory: Path) -> dict:
    cases_file = directory / "cases.txt"
    cases_file.write_text("\n".join(cases) + "\n", encoding="utf-8")
    result = subprocess.run(
        [
            sys.executable,
            str(TOOLS_DIR / "evaluate.py"),
            "--corpus",
            str(corpus),
            "--split",
            "all",
            "--cases-file",
            str(cases_file),
            "--json",
            "--no-build",
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    metrics = json.loads(result.stdout.strip().splitlines()[-1])
    if metrics["failures"] or metrics["missing_expected"] or metrics["missing_cases"]:
        raise RuntimeError(f"fold evaluation failed: {metrics}")
    return metrics


def fold_result(fold: int, train_count: int, cases: list[str], metrics: dict) -> dict:
    worst_text = metrics["worst_cases"][0]
    worst_structure = metrics["worst_structure_cases"][0]
    return {
        "fold": fold,
        "train_cases": train_count,
        "evaluation_cases": len(cases),
        "text_f1": metrics["average_family_f1"],
        "structure_f1": metrics["average_structure_f1"],
        "worst_case": worst_text["case"],
        "worst_case_f1": worst_text["f1"],
        "worst_structure_case": worst_structure["case"],
        "worst_structure_f1": worst_structure["f1"],
        "structural_failures": len(metrics["structural_failures"]),
    }


def export_and_build(weights_file: Path) -> None:
    subprocess.run(
        [
            sys.executable,
            str(TOOLS_DIR / "export_weights.py"),
            "--input",
            str(weights_file),
            "--output",
            str(MODEL_PATH),
        ],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    run(["cargo", "build", "--release", "-p", "rdbl_cli"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=REPO_ROOT / "tests" / "corpus")
    parser.add_argument("--folds", type=int, default=5)
    parser.add_argument("--classification-C", type=float, nargs="+", default=[10.0])
    parser.add_argument("--pairwise-C", type=float, nargs="+", default=[1.0])
    parser.add_argument("--skip-pairwise", action="store_true")
    parser.add_argument("--overlap-threshold", type=float, default=0.5)
    args = parser.parse_args()

    corpus = args.corpus.resolve()
    pairs = load_corpus(corpus, "train") + load_corpus(corpus, "validation")
    pairs = [
        pair
        for pair in pairs
        if (corpus / f"{pair[0].stem}.expected.html").is_file()
    ]
    cases = [path.stem for path, _ in pairs]
    assignments = assign_group_folds(cases, args.folds)
    original_model = MODEL_PATH.read_bytes()

    classification_folds = {C: [] for C in args.classification_C}
    pairwise_folds = {C: [] for C in args.pairwise_C}
    try:
        with tempfile.TemporaryDirectory(prefix="rdbl-cross-validation-") as raw_directory:
            directory = Path(raw_directory)
            classification_data = generate_classification_page_data(
                pairs, args.overlap_threshold
            )
            page_data = None if args.skip_pairwise else generate_pairwise_page_data(pairs)
            for fold in range(args.folds):
                training_cases = {case for case in cases if assignments[case] != fold}
                evaluation_cases = sorted(set(cases) - training_cases)

                X, y, sample_weights = combine_page_data(
                    classification_data, training_cases
                )
                for C in args.classification_C:
                    weights, bias = train_model(
                        X, y, sample_weights=sample_weights, C=C
                    )
                    weights_file = directory / f"fold-{fold}-classification-{C:g}.json"
                    save_weights(weights, bias, weights_file)
                    export_and_build(weights_file)
                    metrics = evaluate_cases(corpus, evaluation_cases, directory)
                    classification_folds[C].append(
                        fold_result(fold, len(training_cases), evaluation_cases, metrics)
                    )

                if page_data is not None:
                    X, y, sample_weights = combine_pairwise_page_data(page_data, training_cases)
                    for C in args.pairwise_C:
                        weights, bias = train_pairwise_ranker(
                            X, y, sample_weights, C=C
                        )
                        weights_file = directory / f"fold-{fold}-pairwise-{C:g}.json"
                        save_weights(weights, bias, weights_file)
                        export_and_build(weights_file)
                        metrics = evaluate_cases(corpus, evaluation_cases, directory)
                        pairwise_folds[C].append(
                            fold_result(
                                fold, len(training_cases), evaluation_cases, metrics
                            )
                        )
    finally:
        MODEL_PATH.write_bytes(original_model)
        run(["cargo", "build", "--release", "-p", "rdbl_cli"])

    print(
        json.dumps(
            {
                "fold_count": args.folds,
                "overlap_threshold": args.overlap_threshold,
                "classifications": {
                    f"{C:g}": {
                        "summary": summarize_fold_metrics(folds),
                        "folds": folds,
                    }
                    for C, folds in classification_folds.items()
                },
                "pairwise": (
                    {
                        f"{C:g}": {
                            "summary": summarize_fold_metrics(folds),
                            "folds": folds,
                        }
                        for C, folds in pairwise_folds.items()
                    }
                    if not args.skip_pairwise
                    else None
                ),
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
