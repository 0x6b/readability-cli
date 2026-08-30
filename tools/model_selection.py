"""Utilities for leakage-resistant model selection."""

import hashlib
import statistics
from collections import defaultdict
from collections.abc import Iterable

from corpus_splits import corpus_family


def assign_group_folds(case_names: Iterable[str], fold_count: int = 5) -> dict[str, int]:
    """Assign complete site families to deterministic, size-balanced folds."""
    if fold_count < 2:
        raise ValueError("fold_count must be at least 2")

    family_cases: dict[str, list[str]] = defaultdict(list)
    for case_name in case_names:
        family_cases[corpus_family(case_name)].append(case_name)

    def family_order(item: tuple[str, list[str]]) -> tuple[int, bytes]:
        family, cases = item
        return -len(cases), hashlib.sha256(family.encode()).digest()

    fold_sizes = [0] * fold_count
    assignments = {}
    for _, cases in sorted(family_cases.items(), key=family_order):
        fold = min(range(fold_count), key=lambda index: (fold_sizes[index], index))
        for case_name in cases:
            assignments[case_name] = fold
        fold_sizes[fold] += len(cases)

    return assignments


def summarize_fold_metrics(folds: list[dict]) -> dict[str, float]:
    """Summarize end-to-end fold metrics without pooling site templates."""
    text_scores = [fold["text_f1"] for fold in folds]
    structure_scores = [fold["structure_f1"] for fold in folds]
    return {
        "mean_text_f1": statistics.fmean(text_scores),
        "text_f1_stddev": statistics.pstdev(text_scores),
        "mean_structure_f1": statistics.fmean(structure_scores),
        "structure_f1_stddev": statistics.pstdev(structure_scores),
        "worst_case_f1": min(fold["worst_case_f1"] for fold in folds),
        "worst_structure_f1": min(fold["worst_structure_f1"] for fold in folds),
        "structural_failures": sum(fold["structural_failures"] for fold in folds),
    }
