#!/usr/bin/env python3
"""
Search for optimal model weights by trying different hyperparameters.

Usage:
    cd tools && uv run search_weights.py --corpus ../tests/corpus
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from itertools import product

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.preprocessing import StandardScaler


# Import from train_logreg.py
sys.path.insert(0, str(Path(__file__).parent))
from train_logreg import (
    FEATURE_NAMES,
    NUM_FEATURES,
    load_corpus,
    generate_training_data,
    save_weights,
)
from corpus_splits import PROTECTED_SPLITS, SPLITS


def train_model(
    X: np.ndarray,
    y: np.ndarray,
    sample_weights: np.ndarray,
    C: float = 1.0,
    class_weight: str | None = None,
) -> tuple[np.ndarray, float]:
    """Train logistic regression model with given params."""
    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    model = LogisticRegression(
        C=C,
        max_iter=2000,
        solver="lbfgs",
        random_state=42,
        class_weight=class_weight,
    )
    model.fit(X_scaled, y, sample_weight=sample_weights)

    weights = model.coef_[0] / scaler.scale_
    bias = model.intercept_[0] - np.dot(weights, scaler.mean_)

    return weights, bias


def train_with_hnm(
    X: np.ndarray,
    y: np.ndarray,
    sample_weights: np.ndarray,
    C: float = 1.0,
    n_iterations: int = 3,
    hard_negative_boost: float = 2.0,
) -> tuple[np.ndarray, float]:
    """Train with hard negative mining."""
    weights = sample_weights.copy()

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    model = None
    for _ in range(n_iterations):
        model = LogisticRegression(
            C=C,
            max_iter=2000,
            solver="lbfgs",
            random_state=42,
        )
        model.fit(X_scaled, y, sample_weight=weights)

        proba = model.predict_proba(X_scaled)[:, 1]
        hard_neg = (y == 0) & (proba > 0.5)
        hard_pos = (y == 1) & (proba < 0.5)
        low_conf = np.abs(proba - 0.5) < 0.2
        hard_mask = hard_neg | hard_pos | low_conf
        weights[hard_mask] *= hard_negative_boost

    final_weights = model.coef_[0] / scaler.scale_
    bias = model.intercept_[0] - np.dot(final_weights, scaler.mean_)

    return final_weights, bias


def export_and_evaluate(
    weights: np.ndarray,
    bias: float,
    corpus_dir: Path,
    evaluation_split: str,
) -> float:
    """Export weights to Rust, rebuild, and evaluate."""
    tools_dir = Path(__file__).parent

    # Save to temp file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        json.dump({
            "weights": weights.tolist(),
            "bias": float(bias),
            "feature_names": FEATURE_NAMES,
        }, f)
        temp_json = f.name

    try:
        # Export to Rust
        result = subprocess.run(
            [sys.executable, "export_weights.py", "--input", temp_json,
             "--output", "../crates/rdbl_core/src/model.rs"],
            cwd=tools_dir,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"Export failed: {result.stderr}")
            return 0.0

        # Rebuild
        result = subprocess.run(
            ["cargo", "build", "--release"],
            cwd=tools_dir.parent,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"Build failed: {result.stderr}")
            return 0.0

        # Evaluate
        result = subprocess.run(
            [
                sys.executable, "evaluate.py",
                "--corpus", str(corpus_dir),
                "--split", evaluation_split,
                "--json",
            ],
            cwd=tools_dir,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"Evaluate failed: {result.stderr}")
            return 0.0

        # Parse result
        for line in result.stdout.strip().split('\n'):
            if line.startswith('{'):
                data = json.loads(line)
                return data.get("average_family_f1", 0.0)
    finally:
        Path(temp_json).unlink(missing_ok=True)

    return 0.0


def main():
    parser = argparse.ArgumentParser(
        description="Search for optimal model weights"
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path("../tests/corpus"),
        help="Corpus directory",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("model_weights_best.json"),
        help="Output file for best weights",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Quick search with fewer combinations",
    )
    parser.add_argument(
        "--train-split",
        choices=SPLITS,
        default="train",
        help="Corpus split used to fit weights (default: train)",
    )
    parser.add_argument(
        "--evaluation-split",
        choices=SPLITS,
        default="validation",
        help="Corpus split used to select weights (default: validation)",
    )

    args = parser.parse_args()

    # Load corpus
    print("Loading corpus...")
    if args.train_split == args.evaluation_split:
        parser.error("training and evaluation splits must differ")
    if args.train_split in PROTECTED_SPLITS or args.evaluation_split in PROTECTED_SPLITS:
        parser.error("regression and holdout splits cannot be used for model fitting or selection")

    corpus_pairs = load_corpus(args.corpus, args.train_split)
    if not corpus_pairs:
        print("No corpus found")
        sys.exit(1)

    # Generate training data with different thresholds
    print("Generating training data...")

    # Search space
    if args.quick:
        C_values = [0.1, 1.0, 10.0]
        threshold_values = [0.4, 0.5]
        hnm_iterations = [0, 3]
        hnm_boost = [2.0]
    else:
        C_values = [0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]
        threshold_values = [0.3, 0.4, 0.5, 0.6]
        hnm_iterations = [0, 2, 3, 5]
        hnm_boost = [1.5, 2.0, 3.0]

    best_score = 0.0
    best_params = {}
    best_weights = None
    best_bias = None

    # Pre-generate training data for each threshold
    threshold_data = {}
    for threshold in threshold_values:
        print(f"  Generating data for threshold={threshold}...")
        X, y, sample_weights = generate_training_data(corpus_pairs, overlap_threshold=threshold)
        threshold_data[threshold] = (X, y, sample_weights)
        print(f"    {len(X)} samples, {sum(y)} positive")

    total_combinations = len(C_values) * len(threshold_values) * len(hnm_iterations)
    print(f"\nSearching {total_combinations} combinations...")

    tried = 0
    for threshold in threshold_values:
        X, y, sample_weights = threshold_data[threshold]

        for C in C_values:
            for hnm_iter in hnm_iterations:
                tried += 1

                # Train
                if hnm_iter == 0:
                    weights, bias = train_model(X, y, sample_weights, C=C)
                    params = {"C": C, "threshold": threshold, "hnm": 0}
                else:
                    for boost in hnm_boost:
                        weights, bias = train_with_hnm(
                            X, y, sample_weights,
                            C=C, n_iterations=hnm_iter, hard_negative_boost=boost
                        )
                        params = {"C": C, "threshold": threshold, "hnm": hnm_iter, "boost": boost}

                        # Evaluate
                        print(f"[{tried}/{total_combinations}] {params}...", end=" ", flush=True)
                        score = export_and_evaluate(
                            weights, bias, args.corpus, args.evaluation_split
                        )
                        print(f"{score:.1%}")

                        if score > best_score:
                            best_score = score
                            best_params = params.copy()
                            best_weights = weights.copy()
                            best_bias = bias
                            print(f"  *** New best: {score:.1%}")
                    continue

                # Evaluate (for non-HNM)
                print(f"[{tried}/{total_combinations}] {params}...", end=" ", flush=True)
                score = export_and_evaluate(
                    weights, bias, args.corpus, args.evaluation_split
                )
                print(f"{score:.1%}")

                if score > best_score:
                    best_score = score
                    best_params = params.copy()
                    best_weights = weights.copy()
                    best_bias = bias
                    print(f"  *** New best: {score:.1%}")

    print(f"\n{'='*60}")
    print(f"Best score: {best_score:.1%}")
    print(f"Best params: {best_params}")

    if best_weights is not None:
        save_weights(
            best_weights,
            best_bias,
            args.output,
            metadata={
                "training_split": args.train_split,
                "selection_split": args.evaluation_split,
                "selection_metric": "family_macro_token_f1",
                "selection_score": best_score,
                "parameters": best_params,
            },
        )

        # Also export to Rust
        print("\nExporting best weights to Rust...")
        subprocess.run(
            [sys.executable, "export_weights.py",
             "--input", str(args.output),
             "--output", "../crates/rdbl_core/src/model.rs"],
            cwd=Path(__file__).parent,
        )
        subprocess.run(
            ["cargo", "build", "--release"],
            cwd=Path(__file__).parent.parent,
        )


if __name__ == "__main__":
    main()
