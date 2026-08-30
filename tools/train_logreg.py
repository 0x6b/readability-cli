#!/usr/bin/env python3
"""
Train logistic regression model for readability extraction.

This script trains a logistic regression model using Readability.js as a teacher.
It generates labeled data by comparing candidate features against teacher output.

Usage:
    cd tools && uv run train_logreg.py --corpus ../tests/corpus --output model_weights.json

Requirements:
    cd tools && pnpm install && uv sync
"""

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Optional

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.preprocessing import StandardScaler
from sklearn.ensemble import GradientBoostingClassifier

from corpus_splits import PROTECTED_SPLITS, SPLITS, corpus_family, in_split
from text_metrics import compute_metrics, extract_text_from_html

# Feature names in order (must match features.rs)
FEATURE_NAMES = [
    # Text quantity (0-7)
    "log_text_len_chars",
    "log_text_len_bytes",
    "log_word_like_count",
    "log_cjk_char_count",
    "whitespace_ratio",
    "line_count",
    "short_line_ratio",
    "avg_line_len",
    # Punctuation (8-15)
    "dot_q_ratio",
    "jp_punct_ratio",
    "comma_ratio",
    "quote_ratio",
    "sentence_end_ratio",
    "colon_ratio",
    "semicolon_ratio",
    "paren_ratio",
    # Link/boilerplate (16-23)
    "log_a_tag_count",
    "log_li_count",
    "log_button_input_form_count",
    "link_density",
    "nav_tag_in_subtree_count",
    "log_form_element_count",
    "log_script_style_count",
    "high_link_density_flag",
    # Code-ness (24-31)
    "log_pre_count",
    "log_code_count",
    "log_code_block_text_len",
    "inline_code_ratio",
    "brace_ratio",
    "backtick_ratio",
    "indent_line_ratio",
    "camel_case_ratio",
    # Structure (32-39)
    "depth_norm",
    "log_subtree_node_count",
    "log_p_count",
    "log_h_count",
    "log_img_count",
    "semantic_main_flag",
    "tag_prior",
    "log_table_count",
    # Keywords (40-47)
    "log_pos_kw_hits",
    "log_neg_kw_hits",
    "pos_minus_neg",
    "has_content_class",
    "has_article_class",
    "has_main_role",
    "has_nav_class",
    "has_sidebar_class",
    # TOC signature (48-55)
    "toc_like",
    "list_link_interaction",
    "log_nested_list_count",
    "list_item_link_ratio",
    "short_text_link_ratio",
    "repetitive_structure",
    "log_ul_ol_count",
    "consecutive_links",
    # Position/context (56-63)
    "relative_position",
    "distance_from_body",
    "has_parent_article",
    "has_parent_main",
    "sibling_content_ratio",
    "child_diversity",
    "text_density",
    "content_to_boilerplate_ratio",
    # Readability-style features (64-71)
    "log_comma_count",
    "avg_paragraph_len",
    "is_article_tag",
    "is_main_tag",
    "is_semantic_container",
    "paragraph_text_score",
    "clean_text_ratio",
    "ancestor_article_depth",
    # Anti-over-extraction features (72-79)
    "content_density",
    "log_clean_text_len",
    "avg_paragraph_len_strict",
    "long_paragraph_ratio",
    "text_to_markup_ratio",
    "has_min_paragraphs",
    "descendant_diversity",
    "content_cluster_score",
]

NUM_FEATURES = 80
REPO_ROOT = Path(__file__).resolve().parent.parent
DEBUG_BINARY = REPO_ROOT / "target" / "debug" / "rdbl"


def run_readability_js(html_path: Path) -> Optional[str]:
    """Run Readability.js on an HTML file and return extracted text."""
    # Uses local node_modules - run `pnpm install` in tools/ first
    tools_dir = Path(__file__).parent
    script = """
    const { Readability } = require('@mozilla/readability');
    const { JSDOM } = require('jsdom');
    const fs = require('fs');

    const html = fs.readFileSync(process.argv[1], 'utf8');
    const dom = new JSDOM(html);
    const reader = new Readability(dom.window.document);
    const article = reader.parse();

    if (article && article.textContent) {
        console.log(article.textContent);
    }
    """

    try:
        result = subprocess.run(
            ["node", "-e", script, str(html_path)],
            capture_output=True,
            text=True,
            timeout=30,
            cwd=tools_dir,  # Run from tools/ to find local node_modules
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return None


def compute_overlap(text1: str, text2: str) -> float:
    """Compute language-aware token F1 between two texts."""
    return compute_metrics(text1, text2)["f1"]


def extract_candidates_with_features(html_path: Path) -> list[dict]:
    """
    Extract candidates and their features from an HTML file.

    This runs the Rust candidate/feature extraction in debug mode
    and parses the JSON output.
    """
    try:
        result = subprocess.run(
            [str(DEBUG_BINARY), "--stdin", "--format", "json", "--debug"],
            input=html_path.read_text(),
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError) as error:
        raise RuntimeError(f"candidate extraction failed for {html_path.name}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RuntimeError(f"candidate extraction failed for {html_path.name}: {detail}")
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"invalid extractor JSON for {html_path.name}: {error}") from error
    if data.get("debug") and data["debug"].get("candidates"):
        return data["debug"]["candidates"]
    return []


def load_corpus(corpus_dir: Path, split: str = "train") -> list[tuple[Path, str]]:
    """
    Load HTML files and their expected teacher outputs.

    Looks for paired files:
    - {name}.html - source HTML
    - {name}.expected.html - expected output (optional, falls back to Readability.js)
    """
    pairs = []

    for html_file in sorted(corpus_dir.glob("*.html")):
        # Skip expected files
        if html_file.stem.endswith(".expected") or not in_split(html_file.stem, split):
            continue

        # Look for expected.html file first
        expected_file = corpus_dir / f"{html_file.stem}.expected.html"
        if expected_file.exists():
            expected_html = expected_file.read_text(encoding="utf-8", errors="replace")
            teacher_text = extract_text_from_html(expected_html)
            if teacher_text:
                pairs.append((html_file, teacher_text))
                print(f"Loaded: {html_file.name} (with expected.html)")
                continue

        # Fall back to Readability.js
        teacher_text = run_readability_js(html_file)
        if teacher_text:
            pairs.append((html_file, teacher_text))
            print(f"Loaded: {html_file.name} (via Readability.js)")

    return pairs


def generate_training_data(
    corpus_pairs: list[tuple[Path, str]],
    overlap_threshold: float = 0.5,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """
    Generate training data from corpus.

    Returns (X, y, weights) where:
    - X is feature matrix [n_samples, n_features]
    - y is labels [n_samples] (1 for positive, 0 for negative)
    - weights is sample weights [n_samples] based on overlap quality

    Labeling strategy:
    - Compute token F1 between each candidate's text and expected text
    - Label as positive if overlap >= threshold
    - Weight samples by overlap quality (squared) to favor high-overlap candidates
    """
    subprocess.run(
        ["cargo", "build", "-p", "rdbl_cli"],
        cwd=REPO_ROOT,
        check=True,
    )

    X_list = []
    y_list = []
    weight_list = []
    family_page_counts = Counter(corpus_family(path.stem) for path, _ in corpus_pairs)

    for html_path, teacher_text in corpus_pairs:
        candidates = extract_candidates_with_features(html_path)

        if not candidates:
            print(f"  {html_path.name}: no candidates")
            continue

        # Compute overlap for each candidate
        positive_count = 0
        page_weight_start = len(weight_list)
        for candidate in candidates:
            # Use the raw feature vector
            feature_vector = candidate.get("feature_vector", [])
            if len(feature_vector) != NUM_FEATURES:
                continue

            X_list.append(np.array(feature_vector))

            # Label based on overlap with expected text
            candidate_text = candidate.get("text", "")
            overlap = compute_overlap(candidate_text, teacher_text)

            is_positive = overlap >= overlap_threshold
            y_list.append(1 if is_positive else 0)

            # Weight by overlap quality (squared to heavily favor high overlap)
            # Positive samples: weight by overlap^2
            # Negative samples: weight by (1-overlap) to penalize near-misses more
            if is_positive:
                weight_list.append(overlap ** 2)
            else:
                weight_list.append((1 - overlap) ** 0.5)  # Sqrt to not over-penalize

            if is_positive:
                positive_count += 1

        # A page with hundreds of candidates must not outweigh a small page, and
        # multiple snapshots of one site must not outweigh a single-site family.
        page_weights = weight_list[page_weight_start:]
        page_weight_total = sum(page_weights)
        family_size = family_page_counts[corpus_family(html_path.stem)]
        if page_weight_total:
            weight_list[page_weight_start:] = [
                weight / page_weight_total / family_size for weight in page_weights
            ]

        print(f"  {html_path.name}: {len(candidates)} candidates, {positive_count} positive")

    return np.array(X_list), np.array(y_list), np.array(weight_list)


def train_model(
    X: np.ndarray,
    y: np.ndarray,
    sample_weights: np.ndarray = None,
    C: float = 1.0,
) -> tuple[np.ndarray, float]:
    """
    Train logistic regression model.

    Returns (weights, bias).
    """
    # Normalize features
    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    # Train logistic regression with L2 regularization
    model = LogisticRegression(
        C=C,
        max_iter=1000,
        solver="lbfgs",
        random_state=42,
    )
    model.fit(X_scaled, y, sample_weight=sample_weights)

    # Return weights and bias
    # Note: In production, we'd need to handle the scaling properly
    # or train on un-normalized features
    weights = model.coef_[0] / scaler.scale_
    bias = model.intercept_[0] - np.dot(weights, scaler.mean_)

    return weights, bias


def train_with_hard_negative_mining(
    X: np.ndarray,
    y: np.ndarray,
    sample_weights: np.ndarray = None,
    C: float = 1.0,
    n_iterations: int = 3,
    hard_negative_boost: float = 2.0,
) -> tuple[np.ndarray, float]:
    """
    Train with hard negative mining - iteratively upweight misclassified samples.

    This improves model performance on difficult cases by:
    1. Training an initial model
    2. Finding samples that are misclassified or have low confidence
    3. Boosting weights of hard samples
    4. Retraining

    Args:
        X: Feature matrix
        y: Labels
        sample_weights: Initial sample weights
        C: Regularization parameter
        n_iterations: Number of hard negative mining iterations
        hard_negative_boost: Weight multiplier for hard samples

    Returns (weights, bias).
    """
    if sample_weights is None:
        sample_weights = np.ones(len(y))
    else:
        sample_weights = sample_weights.copy()

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    model = None

    for iteration in range(n_iterations):
        print(f"  Hard negative mining iteration {iteration + 1}/{n_iterations}")

        # Train model with current weights
        model = LogisticRegression(
            C=C,
            max_iter=1000,
            solver="lbfgs",
            random_state=42 + iteration,
        )
        model.fit(X_scaled, y, sample_weight=sample_weights)

        if iteration < n_iterations - 1:
            # Find hard samples for next iteration
            proba = model.predict_proba(X_scaled)[:, 1]

            # Hard negatives: predicted positive but actually negative
            hard_neg_mask = (y == 0) & (proba > 0.5)

            # Hard positives: predicted negative but actually positive
            hard_pos_mask = (y == 1) & (proba < 0.5)

            # Low confidence correct predictions (also worth boosting)
            low_conf_mask = np.abs(proba - 0.5) < 0.2

            # Boost weights for hard samples
            hard_mask = hard_neg_mask | hard_pos_mask | low_conf_mask
            sample_weights[hard_mask] *= hard_negative_boost

            hard_count = hard_mask.sum()
            print(f"    Found {hard_count} hard samples, boosting weights")

    # Extract final weights
    weights = model.coef_[0] / scaler.scale_
    bias = model.intercept_[0] - np.dot(weights, scaler.mean_)

    return weights, bias


def train_gradient_boosting(
    X: np.ndarray,
    y: np.ndarray,
    sample_weights: np.ndarray = None,
    n_estimators: int = 100,
    max_depth: int = 3,
    learning_rate: float = 0.1,
) -> tuple[np.ndarray, float]:
    """
    Train gradient boosting model and extract linear approximation.

    Gradient boosting can capture non-linear interactions between features.
    We extract a linear approximation via feature importances for use
    in the logistic regression-based Rust code.

    Note: This is an approximation - the actual GB model is non-linear.

    Returns (weights, bias).
    """
    # Normalize features for consistent scale
    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    # Train gradient boosting classifier
    model = GradientBoostingClassifier(
        n_estimators=n_estimators,
        max_depth=max_depth,
        learning_rate=learning_rate,
        random_state=42,
    )
    model.fit(X_scaled, y, sample_weight=sample_weights)

    # Extract feature importances as weights
    # Scale by standard deviation to get proper coefficients
    importances = model.feature_importances_

    # For positive/negative direction, we need to analyze correlation with labels
    # Use sign based on correlation between feature and positive class probability
    from scipy.stats import pearsonr
    proba = model.predict_proba(X_scaled)[:, 1]

    # Compute signed weights based on feature correlation with predictions
    signed_weights = np.zeros_like(importances)
    for i in range(len(importances)):
        corr, _ = pearsonr(X_scaled[:, i], proba)
        signed_weights[i] = importances[i] * np.sign(corr) / scaler.scale_[i]

    # Normalize to similar scale as logistic regression
    # Find a bias that gives ~50% probability at origin
    mean_score = np.dot(signed_weights, scaler.mean_)
    bias = -mean_score  # Adjust bias so mean sample has ~0 score

    print(f"  GB Model accuracy: {model.score(X_scaled, y):.3f}")

    return signed_weights, bias


def save_weights(
    weights: np.ndarray,
    bias: float,
    output_path: Path,
    metadata: dict | None = None,
):
    """Save weights to JSON file."""
    data = {
        "weights": weights.tolist(),
        "bias": float(bias),
        "feature_names": FEATURE_NAMES,
    }
    if metadata is not None:
        data["metadata"] = metadata

    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)

    print(f"Saved weights to {output_path}")


def main():
    parser = argparse.ArgumentParser(
        description="Train logistic regression model for readability extraction"
    )
    parser.add_argument(
        "--corpus",
        type=Path,
        default=Path("tests/corpus"),
        help="Directory containing HTML training files",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("model_weights.json"),
        help="Output file for trained weights",
    )
    parser.add_argument(
        "--split",
        choices=(*SPLITS, "all"),
        default="train",
        help="Corpus split used for training (default: train)",
    )
    parser.add_argument(
        "--C",
        type=float,
        default=1.0,
        help="Regularization parameter (inverse of regularization strength)",
    )
    parser.add_argument(
        "--hard-negative-mining",
        action="store_true",
        help="Enable hard negative mining for improved training",
    )
    parser.add_argument(
        "--hnm-iterations",
        type=int,
        default=3,
        help="Number of hard negative mining iterations",
    )
    parser.add_argument(
        "--gradient-boosting",
        action="store_true",
        help="Use gradient boosting instead of logistic regression",
    )
    parser.add_argument(
        "--gb-estimators",
        type=int,
        default=100,
        help="Number of estimators for gradient boosting",
    )

    args = parser.parse_args()

    if args.split in (*PROTECTED_SPLITS, "all"):
        parser.error("regression, holdout, and all-corpus inputs cannot be used for model fitting")

    if not args.corpus.exists():
        print(f"Corpus directory not found: {args.corpus}")
        print("Creating empty corpus directory...")
        args.corpus.mkdir(parents=True, exist_ok=True)
        print(f"Please add HTML files to {args.corpus} and re-run.")
        sys.exit(1)

    # Load corpus
    print("Loading corpus...")
    corpus_pairs = load_corpus(args.corpus, args.split)

    if not corpus_pairs:
        print("No valid training pairs found.")
        print("Make sure Node.js and @mozilla/readability are installed:")
        print("  cd tools && pnpm install")

        # Generate placeholder weights for development
        print("\nGenerating placeholder weights...")
        weights = np.array([
            # Text quantity (0-7)
            0.8, 0.1, 0.5, 0.3, -0.2, 0.2, -0.8, 0.3,
            # Punctuation (8-15)
            0.4, 0.4, 0.2, 0.1, 0.3, 0.0, 0.0, 0.0,
            # Link/boilerplate (16-23)
            -0.3, -0.5, -0.6, -1.5, -1.0, -0.5, -0.3, -0.1,
            # Code-ness (24-31)
            0.6, 0.3, 0.5, 0.2, 0.1, 0.1, 0.2, 0.1,
            # Structure (32-39)
            -0.3, 0.2, 0.6, 0.4, 0.2, 1.5, 0.8, 0.1,
            # Keywords (40-47)
            0.8, -0.8, 1.0, 0.6, 0.8, 1.2, -1.0, -0.8,
            # TOC signature (48-55)
            -1.5, -1.2, -0.4, -0.8, -0.5, -1.0, -0.3, -0.4,
            # Position/context (56-63)
            -0.2, -0.1, 0.5, 0.6, 0.4, 0.2, 0.3, 0.5,
            # Readability-style (64-71)
            0.5, 0.3, 1.0, 0.8, 0.6, 0.4, 0.5, 0.2,
            # Anti-over-extraction (72-79)
            0.3, 0.2, 0.4, 0.3, 0.2, 0.5, 0.2, 0.3,
        ])
        bias = -2.0
        save_weights(weights, bias, args.output)
        sys.exit(0)

    # Generate training data
    print("Generating training data...")
    X, y, sample_weights = generate_training_data(corpus_pairs)

    if len(X) == 0:
        print("No training data generated.")
        sys.exit(1)

    print(f"Training data: {len(X)} samples, {sum(y)} positive")
    print(f"Sample weights: min={sample_weights.min():.3f}, max={sample_weights.max():.3f}, mean={sample_weights.mean():.3f}")

    # Train model
    print("Training model...")
    if args.gradient_boosting:
        print("Using Gradient Boosting...")
        weights, bias = train_gradient_boosting(
            X, y,
            sample_weights=sample_weights,
            n_estimators=args.gb_estimators,
        )
    elif args.hard_negative_mining:
        print("Using Hard Negative Mining...")
        weights, bias = train_with_hard_negative_mining(
            X, y,
            sample_weights=sample_weights,
            C=args.C,
            n_iterations=args.hnm_iterations,
        )
    else:
        weights, bias = train_model(X, y, sample_weights=sample_weights, C=args.C)

    # Save weights
    save_weights(weights, bias, args.output)

    # Print summary
    print("\nTop positive features:")
    indices = np.argsort(weights)[::-1][:10]
    for i in indices:
        print(f"  {FEATURE_NAMES[i]}: {weights[i]:.3f}")

    print("\nTop negative features:")
    indices = np.argsort(weights)[:10]
    for i in indices:
        print(f"  {FEATURE_NAMES[i]}: {weights[i]:.3f}")


if __name__ == "__main__":
    main()
