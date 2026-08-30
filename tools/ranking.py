"""Pairwise training examples for candidate ranking."""

import numpy as np


def build_pairwise_examples(
    features: list[np.ndarray],
    overlaps: list[float],
    min_gap: float = 0.05,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Compare the best-overlap candidate with materially worse candidates."""
    if len(features) != len(overlaps):
        raise ValueError("features and overlaps must have equal length")
    if not features:
        return np.empty((0, 0)), np.array([], dtype=int), np.array([])

    best_index = max(range(len(overlaps)), key=overlaps.__getitem__)
    best_features = features[best_index]
    best_overlap = overlaps[best_index]
    differences = []
    labels = []
    confidence_weights = []

    for index, (candidate_features, overlap) in enumerate(zip(features, overlaps)):
        gap = best_overlap - overlap
        if index == best_index or gap < min_gap:
            continue
        difference = best_features - candidate_features
        differences.extend((difference, -difference))
        labels.extend((1, 0))
        confidence_weights.extend((gap, gap))

    if not differences:
        return np.empty((0, len(best_features))), np.array([], dtype=int), np.array([])
    return np.asarray(differences), np.asarray(labels), np.asarray(confidence_weights)
