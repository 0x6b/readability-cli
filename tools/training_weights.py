"""Sample weighting rules shared by model training and unit tests."""

import math


def candidate_sample_weight(overlap: float, threshold: float) -> float:
    """Weight candidates by confidence in their teacher-derived label."""
    if overlap >= threshold:
        return overlap**2

    return math.sqrt(1.0 - overlap)
