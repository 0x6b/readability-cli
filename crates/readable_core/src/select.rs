//! Selection logic with guardrails and tie-breakers
//!
//! Selects the best candidate from scored candidates using heuristic rules.

use crate::{
    candidates::Candidate,
    dom::{Arena, NodeId, NodeKind, TagId},
    features::FeatureIndex,
    ExtractOptions,
};

/// Select the best candidate from the list
pub fn select_best<'a>(
    candidates: &'a [Candidate],
    arena: &Arena,
    options: &ExtractOptions,
) -> Option<&'a Candidate> {
    if candidates.is_empty() {
        return None;
    }

    // Step 1: Filter candidates using guardrails
    let filtered: Vec<&Candidate> =
        candidates.iter().filter(|c| passes_guardrails(c, options)).collect();

    if filtered.is_empty() {
        // If all filtered out, fall back to highest scoring candidate
        return candidates
            .iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Step 2: Sort by score descending
    let mut sorted = filtered;
    sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Step 3: Get top-K for tie-breaking
    let top_k: Vec<&Candidate> = sorted.into_iter().take(10).collect();

    if top_k.is_empty() {
        return None;
    }

    // Step 4: Apply tie-breakers if scores are close
    let best = apply_tie_breakers(&top_k, arena, options);

    // Step 5: Consider ancestor fallback
    ancestor_fallback(best, candidates, arena, options)
}

/// Check if a candidate passes the guardrail filters
fn passes_guardrails(candidate: &Candidate, options: &ExtractOptions) -> bool {
    let features = &candidate.features;

    // Get text length from features (exp of log1p - 1)
    let text_len = (features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0) as usize;

    // Get code block text length
    let code_block_len =
        (features.values[FeatureIndex::LogCodeBlockTextLen as usize].exp() - 1.0) as usize;

    // Guardrail 1: Minimum text length (unless code-heavy)
    if text_len < options.min_text_chars {
        // Exception for code-heavy content
        let pre_count = (features.values[FeatureIndex::LogPreCount as usize].exp() - 1.0) as usize;
        let is_code_heavy = code_block_len > 300 && pre_count > 0;

        if !is_code_heavy || !options.code_friendly {
            return false;
        }
    }

    // Guardrail 2: TOC detection
    let toc_like = features.values[FeatureIndex::TocLike as usize];
    let semantic_flag = features.values[FeatureIndex::SemanticMainFlag as usize];

    // High TOC score and not semantic = likely TOC
    if toc_like > 0.5 && semantic_flag < 0.5 {
        return false;
    }

    // Guardrail 3: High link density without content
    let link_density = features.values[FeatureIndex::LinkDensity as usize];
    if link_density > 0.7 && text_len < 500 && code_block_len < 200 {
        return false;
    }

    // Guardrail 4: Nav/sidebar class
    let has_nav_class = features.values[FeatureIndex::HasNavClass as usize] > 0.5;
    let has_sidebar_class = features.values[FeatureIndex::HasSidebarClass as usize] > 0.5;

    if has_nav_class || has_sidebar_class {
        // Allow if it has positive semantic signals overriding
        let has_article_class = features.values[FeatureIndex::HasArticleClass as usize] > 0.5;
        let has_main_role = features.values[FeatureIndex::HasMainRole as usize] > 0.5;

        if !has_article_class && !has_main_role {
            return false;
        }
    }

    true
}

/// Apply tie-breakers when top scores are close
fn apply_tie_breakers<'a>(
    top_k: &[&'a Candidate],
    _arena: &Arena,
    options: &ExtractOptions,
) -> &'a Candidate {
    if top_k.len() == 1 {
        return top_k[0];
    }

    let top1 = top_k[0];
    let top2 = top_k[1];

    // Check if scores are close (within delta)
    const DELTA: f32 = 0.25;
    if (top1.score - top2.score).abs() >= DELTA {
        return top1;
    }

    // Tie-breakers for close scores
    let mut best = top1;
    let mut best_score = compute_tie_break_score(top1, options);

    for candidate in top_k.iter().skip(1) {
        let score = compute_tie_break_score(candidate, options);
        if score > best_score {
            best = candidate;
            best_score = score;
        }
    }

    best
}

/// Compute a tie-break score for a candidate
fn compute_tie_break_score(candidate: &Candidate, options: &ExtractOptions) -> f32 {
    let features = &candidate.features;
    let mut score = 0.0;

    // Prefer semantic containers
    if options.prefer_semantic {
        score += features.values[FeatureIndex::SemanticMainFlag as usize] * 2.0;
    }

    // Prefer lower TOC-like score
    score -= features.values[FeatureIndex::TocLike as usize] * 1.5;

    // Prefer lower link density (but not if code-heavy)
    let code_block_len =
        (features.values[FeatureIndex::LogCodeBlockTextLen as usize].exp() - 1.0) as f32;
    if code_block_len < 500.0 {
        score -= features.values[FeatureIndex::LinkDensity as usize] * 1.0;
    }

    // Prefer larger text
    score += features.values[FeatureIndex::LogTextLenChars as usize] * 0.1;

    // Prefer more paragraphs
    score += features.values[FeatureIndex::LogPCount as usize] * 0.2;

    // Prefer positive keywords
    score += features.values[FeatureIndex::PosMinusNeg as usize] * 0.5;

    score
}

/// Consider promoting to a larger ancestor container
fn ancestor_fallback<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    arena: &Arena,
    _options: &ExtractOptions,
) -> Option<&'a Candidate> {
    // Only consider fallback for small containers like <p>
    let should_fallback = matches!(best.tag, TagId::P | TagId::Li | TagId::Td);

    if !should_fallback {
        return Some(best);
    }

    // Get text length
    let best_text_len =
        (best.features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0) as usize;

    // If already substantial, don't fallback
    if best_text_len > 500 {
        return Some(best);
    }

    // Find the best ancestor candidate - check up to 3 levels
    let mut current_id = best.node_id;
    let mut best_ancestor: Option<&Candidate> = None;
    let mut best_ancestor_score = f32::NEG_INFINITY;

    for _level in 0..3 {
        let parent_id = match arena.get(current_id).and_then(|n| n.parent) {
            Some(id) => id,
            None => break,
        };

        // Look for parent in candidates
        for candidate in candidates {
            if candidate.node_id == parent_id {
                // Check if parent is suitable
                let parent_text_len =
                    (candidate.features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0)
                        as usize;

                // Parent should have substantially more text
                if parent_text_len > best_text_len * 3 {
                    // Check parent tag is suitable
                    if matches!(
                        candidate.tag,
                        TagId::Div | TagId::Section | TagId::Article | TagId::Main
                    ) {
                        // Use tie-break score that favors more paragraphs
                        let p_count = candidate.features.values[FeatureIndex::LogPCount as usize];
                        let tie_score = candidate.score + p_count * 0.5;

                        if tie_score > best_ancestor_score {
                            best_ancestor = Some(candidate);
                            best_ancestor_score = tie_score;
                        }
                    }
                }
            }
        }

        current_id = parent_id;
    }

    // Accept ancestor if its adjusted score is reasonably close to best
    // More generous threshold since we're promoting to a larger container
    if let Some(ancestor) = best_ancestor {
        let adjusted_best_score = best.score;
        // Allow ancestor if it's within 3.0 points of the best
        // This is more generous because larger containers naturally score lower
        if best_ancestor_score > adjusted_best_score - 3.0 {
            return Some(ancestor);
        }
    }

    Some(best)
}

/// Check if a node is inside a boilerplate ancestor
pub fn has_boilerplate_ancestor(arena: &Arena, node_id: NodeId) -> bool {
    for ancestor_id in arena.ancestors(node_id) {
        if let Some(ancestor) = arena.get(ancestor_id) {
            if let NodeKind::Element { tag, .. } = &ancestor.kind {
                if tag.is_boilerplate() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(node_id: NodeId, tag: TagId, score: f32) -> Candidate {
        let mut c = Candidate::new(node_id, tag, 1, 1000, false);
        c.score = score;
        c.features.values[FeatureIndex::LogTextLenChars as usize] = (1001.0_f32).ln(); // log1p(1000)
        c
    }

    #[test]
    fn test_select_best_single() {
        let candidates = vec![make_candidate(1, TagId::Article, 2.0)];
        let arena = crate::dom::Arena::new();
        let options = ExtractOptions::default();

        let best = select_best(&candidates, &arena, &options);
        assert!(best.is_some());
        assert_eq!(best.unwrap().node_id, 1);
    }

    #[test]
    fn test_select_best_highest_score() {
        let candidates = vec![
            make_candidate(1, TagId::Div, 1.0),
            make_candidate(2, TagId::Article, 3.0),
            make_candidate(3, TagId::Section, 2.0),
        ];
        let arena = crate::dom::Arena::new();
        let options = ExtractOptions::default();

        let best = select_best(&candidates, &arena, &options);
        assert!(best.is_some());
        assert_eq!(best.unwrap().node_id, 2);
    }

    #[test]
    fn test_guardrail_filters_small_text() {
        let mut c = make_candidate(1, TagId::Div, 2.0);
        c.features.values[FeatureIndex::LogTextLenChars as usize] = (51.0_f32).ln(); // log1p(50)

        let options = ExtractOptions::default();
        assert!(!passes_guardrails(&c, &options));
    }

    #[test]
    fn test_guardrail_allows_code_heavy() {
        let mut c = make_candidate(1, TagId::Pre, 2.0);
        c.features.values[FeatureIndex::LogTextLenChars as usize] = (51.0_f32).ln();
        c.features.values[FeatureIndex::LogCodeBlockTextLen as usize] = (401.0_f32).ln();
        c.features.values[FeatureIndex::LogPreCount as usize] = (2.0_f32).ln();

        let options = ExtractOptions::default();
        assert!(passes_guardrails(&c, &options));
    }
}
