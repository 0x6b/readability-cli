"""Deterministic, family-aware corpus splits for training and evaluation."""

import hashlib
import re


SPLITS = ("train", "validation", "test")

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
    """Assign a stable 70/15/15 split using a hash of the fixture family."""
    family = corpus_family(case_name)
    bucket = int.from_bytes(hashlib.sha256(family.encode()).digest()[:8], "big") % 100
    if bucket < 70:
        return "train"
    if bucket < 85:
        return "validation"
    return "test"


def in_split(case_name: str, split: str) -> bool:
    if split == "all":
        return True
    if split not in SPLITS:
        raise ValueError(f"unknown corpus split: {split}")
    return corpus_split(case_name) == split
