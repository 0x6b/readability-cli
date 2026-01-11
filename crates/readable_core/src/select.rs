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

    // Step 4.5: Refine overly broad containers
    let best = refine_overextracted(best, candidates, arena, options);

    // Step 4.55: Refine multi-column sections to the most content-rich child
    let best = refine_columnar_children(best, candidates, arena, options);

    // Step 4.6: Promote step-based containers
    let best = promote_step_container(best, candidates, arena);

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
    // Only compare candidates within DELTA of the top score to avoid
    // promoting much larger containers with significantly lower logits.
    let threshold = top1.score - DELTA;
    let mut best = top1;
    let mut best_score = compute_tie_break_score(top1, options);

    for candidate in top_k.iter().skip(1) {
        if candidate.score < threshold {
            continue;
        }
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

fn refine_overextracted<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    arena: &Arena,
    options: &ExtractOptions,
) -> &'a Candidate {
    let mut current = best;
    for _ in 0..2 {
        let refined = refine_overextracted_once(current, candidates, arena, options);
        if refined.node_id == current.node_id {
            break;
        }
        current = refined;
    }
    current
}

fn refine_overextracted_once<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    arena: &Arena,
    options: &ExtractOptions,
) -> &'a Candidate {
    let best_text_len =
        (best.features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0) as usize;
    if best_text_len < options.min_text_chars * 4 {
        return best;
    }

    if !matches!(best.tag, TagId::Div | TagId::Section | TagId::Main | TagId::Article) {
        return best;
    }

    let best_toc = best.features.values[FeatureIndex::TocLike as usize];
    let best_cluster = best.features.values[FeatureIndex::ContentClusterScore as usize];
    let best_link_density = best.features.values[FeatureIndex::LinkDensity as usize];
    let best_clean_ratio = best.features.values[FeatureIndex::CleanTextRatio as usize];
    let best_p_count =
        (best.features.values[FeatureIndex::LogPCount as usize].exp() - 1.0) as usize;
    if best_toc < 0.2 && best_cluster > 0.4 {
        return best;
    }

    if matches!(best.tag, TagId::Main | TagId::Div)
        && (best_toc > 0.25 || best_link_density > 0.15 || best_clean_ratio < 0.85)
    {
        let mut semantic_refined: Option<&Candidate> = None;
        let mut semantic_score = f32::NEG_INFINITY;

        for candidate in candidates {
            if candidate.node_id == best.node_id {
                continue;
            }
            if candidate.tag != TagId::Article {
                continue;
            }
            if !is_descendant(arena, candidate.node_id, best.node_id) {
                continue;
            }

            let text_len = (candidate.features.values[FeatureIndex::LogTextLenChars as usize].exp()
                - 1.0) as usize;
            let ratio = text_len as f32 / best_text_len.max(1) as f32;
            if ratio < 0.6 || ratio > 0.95 {
                continue;
            }

            let cluster = candidate.features.values[FeatureIndex::ContentClusterScore as usize];
            if cluster < 0.8 {
                continue;
            }

            let toc_like = candidate.features.values[FeatureIndex::TocLike as usize];
            if toc_like > best_toc {
                continue;
            }

            let link_density = candidate.features.values[FeatureIndex::LinkDensity as usize];
            let clean_ratio = candidate.features.values[FeatureIndex::CleanTextRatio as usize];
            if link_density > best_link_density - 0.02 && clean_ratio < best_clean_ratio + 0.02 {
                continue;
            }

            let score = cluster * 1.5 + clean_ratio - toc_like - link_density + ratio;
            if score > semantic_score {
                semantic_refined = Some(candidate);
                semantic_score = score;
            }
        }

        if let Some(candidate) = semantic_refined {
            return candidate;
        }
    }

    let mut refined: Option<&Candidate> = None;
    let mut refined_score = f32::NEG_INFINITY;
    let allow_smaller = best_cluster < 0.35 || best_toc > 0.3;

    for candidate in candidates {
        if candidate.node_id == best.node_id {
            continue;
        }
        if !is_descendant(arena, candidate.node_id, best.node_id) {
            continue;
        }

        if !matches!(candidate.tag, TagId::Div | TagId::Section | TagId::Main | TagId::Article) {
            continue;
        }

        let text_len = (candidate.features.values[FeatureIndex::LogTextLenChars as usize].exp()
            - 1.0) as usize;
        if text_len < options.min_text_chars * 2 {
            continue;
        }

        let has_min_paragraphs =
            candidate.features.values[FeatureIndex::HasMinParagraphs as usize] > 0.5;
        if !has_min_paragraphs {
            continue;
        }

        let ratio = text_len as f32 / best_text_len.max(1) as f32;
        let min_ratio = if allow_smaller { 0.15 } else { 0.2 };
        if ratio < min_ratio || ratio > 0.6 {
            continue;
        }

        let cluster = candidate.features.values[FeatureIndex::ContentClusterScore as usize];
        if cluster < 0.8 {
            continue;
        }

        let toc_like = candidate.features.values[FeatureIndex::TocLike as usize];
        if toc_like > best_toc {
            continue;
        }

        let clean_ratio = candidate.features.values[FeatureIndex::CleanTextRatio as usize];
        if ratio < 0.2 && (cluster < 0.9 || clean_ratio < best_clean_ratio + 0.03) {
            continue;
        }

        let avg_para_len_strict =
            candidate.features.values[FeatureIndex::AvgParagraphLenStrict as usize];
        let long_para_ratio = candidate.features.values[FeatureIndex::LongParagraphRatio as usize];
        let focus_score = cluster * 2.0
            + clean_ratio
            - toc_like
            - ratio * 0.2
            + avg_para_len_strict * 0.3
            + long_para_ratio * 0.2;

        if focus_score > refined_score {
            refined = Some(candidate);
            refined_score = focus_score;
        }
    }

    if best_toc < 0.15
        && best_link_density < 0.1
        && best_clean_ratio > 0.9
        && best_p_count >= 8
    {
        return best;
    }

    refined.unwrap_or(best)
}

fn refine_columnar_children<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    arena: &Arena,
    options: &ExtractOptions,
) -> &'a Candidate {
    if !matches!(best.tag, TagId::Section | TagId::Div) {
        return best;
    }

    let best_text_len =
        (best.features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0) as usize;
    if best_text_len < options.min_text_chars * 3 {
        return best;
    }

    let best_toc = best.features.values[FeatureIndex::TocLike as usize];
    let best_link_density = best.features.values[FeatureIndex::LinkDensity as usize];
    let best_clean_ratio = best.features.values[FeatureIndex::CleanTextRatio as usize];
    if best_toc < 0.15 && best_link_density < 0.1 && best_clean_ratio > 0.9 {
        return best;
    }

    let best_li_count =
        (best.features.values[FeatureIndex::LogLiCount as usize].exp() - 1.0) as usize;
    if best_li_count < 6 {
        return best;
    }

    let mut scored_children: Vec<(&Candidate, f32, f32)> = Vec::new();

    for candidate in candidates {
        if candidate.node_id == best.node_id {
            continue;
        }

        if !matches!(candidate.tag, TagId::Div | TagId::Section | TagId::Article) {
            continue;
        }

        if arena.get(candidate.node_id).and_then(|n| n.parent) != Some(best.node_id) {
            continue;
        }

        let text_len =
            (candidate.features.values[FeatureIndex::LogTextLenChars as usize].exp() - 1.0)
                as usize;
        if text_len < options.min_text_chars * 2 {
            continue;
        }

        let ratio = text_len as f32 / best_text_len.max(1) as f32;
        if ratio < 0.25 || ratio > 0.8 {
            continue;
        }

        let has_min_paragraphs =
            candidate.features.values[FeatureIndex::HasMinParagraphs as usize] > 0.5;
        if !has_min_paragraphs {
            continue;
        }

        let avg_para_len_strict =
            candidate.features.values[FeatureIndex::AvgParagraphLenStrict as usize];
        let long_para_ratio = candidate.features.values[FeatureIndex::LongParagraphRatio as usize];
        let clean_ratio = candidate.features.values[FeatureIndex::CleanTextRatio as usize];
        let toc_like = candidate.features.values[FeatureIndex::TocLike as usize];
        let cluster = candidate.features.values[FeatureIndex::ContentClusterScore as usize];

        let score = avg_para_len_strict * 0.8
            + long_para_ratio * 0.4
            + clean_ratio
            + cluster
            - toc_like
            - ratio * 0.1;

        scored_children.push((candidate, score, avg_para_len_strict));
    }

    if scored_children.len() < 2 {
        return best;
    }

    scored_children.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let (best_child, best_score, best_avg) = scored_children[0];
    let (_, runner_score, runner_avg) = scored_children[1];

    if best_score >= runner_score + 0.05 || best_avg >= runner_avg + 0.08 {
        return best_child;
    }

    best
}

fn promote_step_container<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    arena: &Arena,
) -> &'a Candidate {
    if best.tag != TagId::Div {
        return best;
    }

    let p_count = (best.features.values[FeatureIndex::LogPCount as usize].exp() - 1.0) as usize;
    if p_count > 1 {
        return best;
    }

    let parent_id = match arena.get(best.node_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return best,
    };

    let class_id = arena
        .get_attributes(parent_id)
        .map(|attrs| {
            format!(
                "{} {}",
                attrs.class.as_deref().unwrap_or(""),
                attrs.id.as_deref().unwrap_or("")
            )
            .to_lowercase()
        })
        .unwrap_or_default();

    if !class_id.contains("step") && !class_id.contains("instruction") {
        return best;
    }

    candidates
        .iter()
        .find(|candidate| candidate.node_id == parent_id)
        .unwrap_or(best)
}

fn is_descendant(arena: &Arena, node_id: NodeId, ancestor_id: NodeId) -> bool {
    for parent_id in arena.ancestors(node_id) {
        if parent_id == ancestor_id {
            return true;
        }
    }
    false
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
