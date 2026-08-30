"""Deterministic corpus partitions with explicitly protected evaluation cases."""

import hashlib
import json
import re
from pathlib import Path


SPLITS = ("train", "validation", "regression", "holdout")
PROTECTED_SPLITS = ("regression", "holdout")

# These cases were previously called the test split, but have been inspected and
# used during development. Keep them as a stable regression suite rather than
# presenting them as unseen performance evidence.
REGRESSION_CASES = frozenset(
    {
        "antirez_com_news_158",
        "blog_val_town_blog_codegen",
        "breitbart",
        "claytonwramsey_com_blog_dumpster2",
        "ehow-1",
        "ehow-2",
        "embedded-videos",
        "ftr-youtube",
        "gmw",
        "google-sre-book-1",
        "links-in-tables",
        "maximullaris_com_program-in-awk_html",
        "mercurial",
        "news_yahoo_co_jp_articles_40ea598367a4c9ca44b11a62_4ca4b0cd",
        "quanta-1",
        "replace-font-tags",
        "seattletimes-1",
        "simplyfound-1",
        "www_gingerbill_org_article_2026_01_11_mitigating-t_7dcf6ed9",
    }
)

# The manifest records provenance without requiring anyone to inspect extractor
# output. Once evaluated, move its cases to REGRESSION_CASES and replace it with
# a newly collected manifest.
HOLDOUT_MANIFEST = json.loads(
    (Path(__file__).parent / "holdout_manifest.json").read_text(encoding="utf-8")
)
HOLDOUT_CASES = frozenset(entry["case"] for entry in HOLDOUT_MANIFEST["cases"])

# Keep variants from the same site or fixture family in one split. This avoids
# learning a site's template in one split and measuring it in another.
FAMILY_MARKERS = (
    "ascii_jp_elem_",
    "base-url",
    "cnet",
    "cursor_com_",
    "gigazine_net_",
    "hatenablog_com_",
    "lifehacker-",
    "note_com_",
    "wikipedia",
    "www_cursor_com_",
    "zenn_dev_",
)


def corpus_family(case_name: str) -> str:
    for marker in FAMILY_MARKERS:
        if marker in case_name:
            return marker.rstrip("_-")
    return re.sub(r"-\d+$", "", case_name)


def corpus_split(case_name: str) -> str:
    """Assign protected cases explicitly and development data by family hash."""
    if case_name in HOLDOUT_CASES:
        return "holdout"
    if case_name in REGRESSION_CASES:
        return "regression"

    family = corpus_family(case_name)
    bucket = int.from_bytes(hashlib.sha256(family.encode()).digest()[:8], "big") % 100
    if bucket < 70:
        return "train"
    return "validation"


def in_split(case_name: str, split: str) -> bool:
    if split == "all":
        return True
    if split not in SPLITS:
        raise ValueError(f"unknown corpus split: {split}")
    return corpus_split(case_name) == split
