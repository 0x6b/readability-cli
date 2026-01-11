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
from pathlib import Path
from typing import Optional

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.preprocessing import StandardScaler

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
    "inline_style_count",
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
]

NUM_FEATURES = 64


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
    """Compute Jaccard similarity between two texts based on word sets."""
    words1 = set(text1.lower().split())
    words2 = set(text2.lower().split())

    if not words1 or not words2:
        return 0.0

    intersection = len(words1 & words2)
    union = len(words1 | words2)

    return intersection / union if union > 0 else 0.0


def extract_candidates_with_features(html_path: Path) -> list[dict]:
    """
    Extract candidates and their features from an HTML file.

    This runs the Rust candidate/feature extraction in debug mode
    and parses the JSON output.
    """
    try:
        result = subprocess.run(
            ["cargo", "run", "--", "--stdin", "--format", "json", "--debug"],
            input=html_path.read_text(),
            capture_output=True,
            text=True,
            timeout=60,
            cwd=html_path.parent.parent.parent,  # repo root
        )
        if result.returncode == 0:
            data = json.loads(result.stdout)
            if data.get("debug") and data["debug"].get("candidates"):
                return data["debug"]["candidates"]
    except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
        pass
    return []


def extract_text_from_html(html: str) -> str:
    """Extract plain text from HTML content."""
    # Simple text extraction - strip tags
    import re

    # Remove script/style content
    html = re.sub(r"<(script|style)[^>]*>.*?</\1>", "", html, flags=re.DOTALL | re.IGNORECASE)
    # Remove tags
    text = re.sub(r"<[^>]+>", " ", html)
    # Normalize whitespace
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def load_corpus(corpus_dir: Path) -> list[tuple[Path, str]]:
    """
    Load HTML files and their expected teacher outputs.

    Looks for paired files:
    - {name}.html - source HTML
    - {name}.expected.html - expected output (optional, falls back to Readability.js)
    """
    pairs = []

    for html_file in corpus_dir.glob("*.html"):
        # Skip expected files
        if html_file.stem.endswith(".expected"):
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
) -> tuple[np.ndarray, np.ndarray]:
    """
    Generate training data from corpus.

    Returns (X, y) where:
    - X is feature matrix [n_samples, n_features]
    - y is labels [n_samples] (1 for positive, 0 for negative)

    Labeling strategy:
    - Compute Jaccard overlap between each candidate's text and expected text
    - Label as positive if overlap >= threshold
    - This teaches the model to match Readability.js output
    """
    X_list = []
    y_list = []

    for html_path, teacher_text in corpus_pairs:
        candidates = extract_candidates_with_features(html_path)

        if not candidates:
            print(f"  {html_path.name}: no candidates")
            continue

        # Compute overlap for each candidate
        positive_count = 0
        for candidate in candidates:
            # Use the raw 64-element feature vector
            feature_vector = candidate.get("feature_vector", [])
            if len(feature_vector) != NUM_FEATURES:
                continue

            X_list.append(np.array(feature_vector))

            # Label based on overlap with expected text
            candidate_text = candidate.get("text", "")
            overlap = compute_overlap(candidate_text, teacher_text)

            is_positive = overlap >= overlap_threshold
            y_list.append(1 if is_positive else 0)
            if is_positive:
                positive_count += 1

        print(f"  {html_path.name}: {len(candidates)} candidates, {positive_count} positive")

    return np.array(X_list), np.array(y_list)


def train_model(
    X: np.ndarray,
    y: np.ndarray,
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
    model.fit(X_scaled, y)

    # Return weights and bias
    # Note: In production, we'd need to handle the scaling properly
    # or train on un-normalized features
    weights = model.coef_[0] / scaler.scale_
    bias = model.intercept_[0] - np.dot(weights, scaler.mean_)

    return weights, bias


def save_weights(
    weights: np.ndarray,
    bias: float,
    output_path: Path,
):
    """Save weights to JSON file."""
    data = {
        "weights": weights.tolist(),
        "bias": float(bias),
        "feature_names": FEATURE_NAMES,
    }

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
        "--C",
        type=float,
        default=1.0,
        help="Regularization parameter (inverse of regularization strength)",
    )

    args = parser.parse_args()

    if not args.corpus.exists():
        print(f"Corpus directory not found: {args.corpus}")
        print("Creating empty corpus directory...")
        args.corpus.mkdir(parents=True, exist_ok=True)
        print(f"Please add HTML files to {args.corpus} and re-run.")
        sys.exit(1)

    # Load corpus
    print("Loading corpus...")
    corpus_pairs = load_corpus(args.corpus)

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
        ])
        bias = -2.0
        save_weights(weights, bias, args.output)
        sys.exit(0)

    # Generate training data
    print("Generating training data...")
    X, y = generate_training_data(corpus_pairs)

    if len(X) == 0:
        print("No training data generated.")
        sys.exit(1)

    print(f"Training data: {len(X)} samples, {sum(y)} positive")

    # Train model
    print("Training model...")
    weights, bias = train_model(X, y, C=args.C)

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
