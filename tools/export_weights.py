#!/usr/bin/env python3
"""
Export trained model weights to Rust const arrays.

This script reads the trained weights from JSON and generates
Rust code that can be embedded in model.rs.

Usage:
    python export_weights.py --input model_weights.json --output weights.rs
    # Or copy-paste the output into crates/readable_core/src/model.rs
"""

import argparse
import json
from pathlib import Path

# Feature group comments for documentation
FEATURE_GROUPS = [
    ("Text quantity", 0, 8),
    ("Punctuation", 8, 16),
    ("Link/boilerplate", 16, 24),
    ("Code-ness", 24, 32),
    ("Structure", 32, 40),
    ("Keywords", 40, 48),
    ("TOC signature", 48, 56),
    ("Position/context", 56, 64),
]

# Feature names for comments
FEATURE_NAMES = [
    # Text quantity (0-7)
    "LogTextLenChars",
    "LogTextLenBytes",
    "LogWordLikeCount",
    "LogCjkCharCount",
    "WhitespaceRatio",
    "LineCount",
    "ShortLineRatio",
    "AvgLineLen",
    # Punctuation (8-15)
    "DotQRatio",
    "JpPunctRatio",
    "CommaRatio",
    "QuoteRatio",
    "SentenceEndRatio",
    "ColonRatio",
    "SemicolonRatio",
    "ParenRatio",
    # Link/boilerplate (16-23)
    "LogATagCount",
    "LogLiCount",
    "LogButtonInputFormCount",
    "LinkDensity",
    "NavTagInSubtreeCount",
    "LogFormElementCount",
    "LogScriptStyleCount",
    "InlineStyleCount",
    # Code-ness (24-31)
    "LogPreCount",
    "LogCodeCount",
    "LogCodeBlockTextLen",
    "InlineCodeRatio",
    "BraceRatio",
    "BacktickRatio",
    "IndentLineRatio",
    "CamelCaseRatio",
    # Structure (32-39)
    "DepthNorm",
    "LogSubtreeNodeCount",
    "LogPCount",
    "LogHCount",
    "LogImgCount",
    "SemanticMainFlag",
    "TagPrior",
    "LogTableCount",
    # Keywords (40-47)
    "LogPosKwHits",
    "LogNegKwHits",
    "PosMinusNeg",
    "HasContentClass",
    "HasArticleClass",
    "HasMainRole",
    "HasNavClass",
    "HasSidebarClass",
    # TOC signature (48-55)
    "TocLike",
    "ListLinkInteraction",
    "LogNestedListCount",
    "ListItemLinkRatio",
    "ShortTextLinkRatio",
    "RepetitiveStructure",
    "LogUlOlCount",
    "ConsecutiveLinks",
    # Position/context (56-63)
    "RelativePosition",
    "DistanceFromBody",
    "HasParentArticle",
    "HasParentMain",
    "SiblingContentRatio",
    "ChildDiversity",
    "TextDensity",
    "ContentToBoilerplateRatio",
]


def format_weight(w: float) -> str:
    """Format a weight value for Rust."""
    if w == 0:
        return "0.0"
    elif w == int(w):
        return f"{int(w)}.0"
    else:
        return f"{w:.4f}".rstrip("0").rstrip(".")
        # Ensure at least one decimal place
        if "." not in result:
            result += ".0"
        return result


def generate_rust_code(weights: list[float], bias: float) -> str:
    """Generate Rust const array code."""
    lines = []

    lines.append("/// Model weights - trained via tools/train_logreg.py")
    lines.append("const WEIGHTS: [f32; NUM_FEATURES] = [")

    # Output weights with group comments
    for group_name, start, end in FEATURE_GROUPS:
        lines.append(f"    // {group_name} ({start}-{end-1})")
        for i in range(start, end):
            w = weights[i]
            name = FEATURE_NAMES[i]
            formatted = format_weight(w)

            # Add comma except for last element
            comma = "," if i < len(weights) - 1 else ""

            # Pad for alignment
            lines.append(f"    {formatted}{comma}  // {name}")

    lines.append("];")
    lines.append("")
    lines.append(f"/// Model bias term")
    lines.append(f"const BIAS: f32 = {format_weight(bias)};")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Export trained weights to Rust const arrays"
    )
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("model_weights.json"),
        help="Input JSON file with trained weights",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output Rust file (or stdout if not specified)",
    )

    args = parser.parse_args()

    # Load weights
    with open(args.input) as f:
        data = json.load(f)

    weights = data["weights"]
    bias = data["bias"]

    if len(weights) != 64:
        print(f"Warning: Expected 64 weights, got {len(weights)}")

    # Generate Rust code
    rust_code = generate_rust_code(weights, bias)

    # Output
    if args.output:
        args.output.write_text(rust_code)
        print(f"Written to {args.output}")
    else:
        print(rust_code)
        print("\n// Copy the above into crates/readable_core/src/model.rs")


if __name__ == "__main__":
    main()
