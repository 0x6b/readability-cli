//! Selection logic with guardrails and tie-breakers
//!
//! Selects the best candidate from scored candidates using heuristic rules.

use crate::{
    ExtractOptions,
    candidates::Candidate,
    dom::{Arena, NodeId, TagId},
    features::FeatureIndex,
    tags::{is_content_container, is_refinement_container, is_small_content_tag},
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

    // Step 4.25: Rescue a substantive semantic article from a short summary candidate
    let best = semantic_article_fallback(best, candidates, options);

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

    // Get text length from features
    let text_len = features.get_count(FeatureIndex::LogTextLenChars);

    // Get code block text length
    let code_block_len = features.get_count(FeatureIndex::LogCodeBlockTextLen);

    // Guardrail 1: Minimum text length (unless code-heavy)
    if text_len < options.min_text_chars {
        // Exception for code-heavy content
        let pre_count = features.get_count(FeatureIndex::LogPreCount);
        let is_code_heavy = code_block_len > 300 && pre_count > 0;

        if !is_code_heavy || !options.code_friendly {
            return false;
        }
    }

    // Guardrail 2: TOC detection
    let toc_like = features.get(FeatureIndex::TocLike);
    let semantic_flag = features.get(FeatureIndex::SemanticMainFlag);

    // High TOC score and not semantic = likely TOC
    if toc_like > 0.5 && semantic_flag < 0.5 {
        return false;
    }

    // Guardrail 3: High link density without content
    let link_density = features.get(FeatureIndex::LinkDensity);
    if link_density > 0.7 && text_len < 500 && code_block_len < 200 {
        return false;
    }

    // Guardrail 4: Nav/sidebar class
    let has_nav_class = features.get(FeatureIndex::HasNavClass) > 0.5;
    let has_sidebar_class = features.get(FeatureIndex::HasSidebarClass) > 0.5;

    if has_nav_class || has_sidebar_class {
        // Allow if it has positive semantic signals overriding
        let has_article_class = features.get(FeatureIndex::HasArticleClass) > 0.5;
        let has_main_role = features.get(FeatureIndex::HasMainRole) > 0.5;

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
        score += features.get(FeatureIndex::SemanticMainFlag) * 2.0;
    }

    // Prefer lower TOC-like score
    score -= features.get(FeatureIndex::TocLike) * 1.5;

    // Prefer lower link density (but not if code-heavy)
    let code_block_len = features.get_count(FeatureIndex::LogCodeBlockTextLen) as f32;
    if code_block_len < 500.0 {
        score -= features.get(FeatureIndex::LinkDensity) * 1.0;
    }

    // Prefer larger text
    score += features.get(FeatureIndex::LogTextLenChars) * 0.1;

    // Prefer more paragraphs
    score += features.get(FeatureIndex::LogPCount) * 0.2;

    // Prefer positive keywords
    score += features.get(FeatureIndex::PosMinusNeg) * 0.5;

    score
}

fn semantic_article_fallback<'a>(
    best: &'a Candidate,
    candidates: &'a [Candidate],
    options: &ExtractOptions,
) -> &'a Candidate {
    if !options.prefer_semantic {
        return best;
    }

    let best_text_len = best.features.get_count(FeatureIndex::LogTextLenChars);
    let best_is_semantic = best.features.get(FeatureIndex::SemanticMainFlag) > 0.5;
    if best_is_semantic || best_text_len >= options.min_text_chars * 5 {
        return best;
    }

    candidates
        .iter()
        .filter(|candidate| {
            let features = &candidate.features;
            candidate.tag == TagId::Article
                && candidate.score >= best.score - 1.0
                && features.get_count(FeatureIndex::LogTextLenChars) >= best_text_len * 5
                && features.get_count(FeatureIndex::LogPCount) >= 5
                && features.get(FeatureIndex::TocLike) <= 0.5
                && features.get(FeatureIndex::LinkDensity) <= 0.6
        })
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(best)
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
    let best_text_len = best.features.get_count(FeatureIndex::LogTextLenChars);
    if best_text_len < options.min_text_chars * 4 {
        return best;
    }

    if !is_content_container(best.tag) {
        return best;
    }

    let best_toc = best.features.get(FeatureIndex::TocLike);
    let best_cluster = best.features.get(FeatureIndex::ContentClusterScore);
    let best_link_density = best.features.get(FeatureIndex::LinkDensity);
    let best_clean_ratio = best.features.get(FeatureIndex::CleanTextRatio);
    let best_p_count = best.features.get_count(FeatureIndex::LogPCount);

    if best.tag == TagId::Main {
        let focused_children: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| {
                let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
                let ratio = text_len as f32 / best_text_len.max(1) as f32;
                matches!(candidate.tag, TagId::Article | TagId::Section)
                    && arena.get(candidate.node_id).and_then(|node| node.parent)
                        == Some(best.node_id)
                    && (0.6..=0.9).contains(&ratio)
                    && candidate.features.get(FeatureIndex::SemanticMainFlag) > 0.5
                    && candidate.features.get_count(FeatureIndex::LogPosKwHits) > 0
                    && candidate.features.get(FeatureIndex::TocLike) <= best_toc + 0.05
                    && candidate.features.get(FeatureIndex::LinkDensity) <= best_link_density + 0.1
            })
            .collect();
        if let [child] = focused_children.as_slice() {
            return child;
        }
    }

    if matches!(best.tag, TagId::Main | TagId::Div) {
        let direct_articles: Vec<&Candidate> = candidates
            .iter()
            .filter(|candidate| {
                let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
                candidate.tag == TagId::Article
                    && arena.get(candidate.node_id).and_then(|node| node.parent)
                        == Some(best.node_id)
                    && text_len >= options.min_text_chars * 5
                    && best_text_len >= text_len * 20
                    && candidate.features.get_count(FeatureIndex::LogPCount) >= 5
                    && candidate.features.get(FeatureIndex::TocLike) <= 0.1
                    && candidate.features.get(FeatureIndex::LinkDensity) <= 0.2
            })
            .collect();
        if let [article] = direct_articles.as_slice() {
            return article;
        }
    }

    if best.tag == TagId::Article {
        for candidate in candidates {
            if !matches!(candidate.tag, TagId::Div | TagId::Section)
                || arena.get(candidate.node_id).and_then(|node| node.parent) != Some(best.node_id)
            {
                continue;
            }
            let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
            let ratio = text_len as f32 / best_text_len.max(1) as f32;
            let semantic = candidate.features.get(FeatureIndex::SemanticMainFlag) > 0.5;
            let positive_hits = candidate.features.get_count(FeatureIndex::LogPosKwHits);
            let toc_like = candidate.features.get(FeatureIndex::TocLike);
            if (0.75..0.99).contains(&ratio)
                && semantic
                && positive_hits > 0
                && toc_like <= best_toc
            {
                return candidate;
            }
        }
    }

    if matches!(best.tag, TagId::Main | TagId::Div)
        && (best_cluster < 0.4
            || best_toc > 0.25
            || best_link_density > 0.15
            || best_clean_ratio < 0.85)
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

            let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
            let ratio = text_len as f32 / best_text_len.max(1) as f32;
            if !(0.25..=0.95).contains(&ratio) {
                continue;
            }

            let cluster = candidate.features.get(FeatureIndex::ContentClusterScore);
            let toc_like = candidate.features.get(FeatureIndex::TocLike);
            if toc_like > best_toc {
                continue;
            }

            let link_density = candidate.features.get(FeatureIndex::LinkDensity);
            let clean_ratio = candidate.features.get(FeatureIndex::CleanTextRatio);
            let semantic_signal = candidate.features.get(FeatureIndex::SemanticMainFlag) > 0.5
                && candidate.features.get_count(FeatureIndex::LogPosKwHits) > 0
                && toc_like + 0.2 <= best_toc
                && clean_ratio >= best_clean_ratio + 0.1;
            if !semantic_signal && (cluster < 0.45 || cluster < best_cluster + 0.2) {
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

    let mut explicit_bodies = Vec::new();
    let mut explicit_body: Option<&Candidate> = None;
    let mut explicit_body_score = f32::NEG_INFINITY;
    for candidate in candidates {
        if candidate.node_id == best.node_id
            || !is_descendant(arena, candidate.node_id, best.node_id)
        {
            continue;
        }
        let attrs = arena.get_attributes(candidate.node_id);
        let class_id = attrs
            .map(|attrs| attrs.normalized_class_id())
            .unwrap_or_default();
        let schema_article_body = attrs
            .and_then(|attrs| attrs.itemprop.as_deref())
            .is_some_and(|itemprop| {
                itemprop
                    .split_ascii_whitespace()
                    .any(|value| value.eq_ignore_ascii_case("articleBody"))
            });
        let bem_article_body = class_id.contains("article__body");
        if !schema_article_body
            && !class_id.contains("articlebody")
            && !class_id.contains("article-body")
            && !class_id.contains("article_body")
            && !bem_article_body
        {
            continue;
        }

        let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
        let ratio = text_len as f32 / best_text_len.max(1) as f32;
        let clean_ratio = candidate.features.get(FeatureIndex::CleanTextRatio);
        let toc_like = candidate.features.get(FeatureIndex::TocLike);
        if text_len < options.min_text_chars * 2
            || !(0.1..=0.8).contains(&ratio)
            || candidate.features.get(FeatureIndex::HasMinParagraphs) < 0.5
            || toc_like > 0.1
        {
            continue;
        }
        explicit_bodies.push(candidate);

        // BEM article-body classes commonly repeat for chunks split by ads.
        // Use them to rejoin fragments, but not as a sole hard boundary: a
        // single chunk can legitimately be followed by references or notes.
        if bem_article_body {
            continue;
        }

        let min_clean_gain = if schema_article_body { 0.05 } else { 0.1 };
        let direct_substantive_body = arena
            .get(candidate.node_id)
            .is_some_and(|node| node.parent == Some(best.node_id))
            && text_len >= options.min_text_chars * 5
            && candidate.features.get_count(FeatureIndex::LogPCount) >= 10
            && clean_ratio >= 0.85
            && clean_ratio >= best_clean_ratio + 0.03;
        if !direct_substantive_body
            && (clean_ratio < 0.9 || clean_ratio < best_clean_ratio + min_clean_gain)
        {
            continue;
        }

        let score = clean_ratio - toc_like - ratio * 0.1;
        if score > explicit_body_score {
            explicit_body = Some(candidate);
            explicit_body_score = score;
        }
    }
    if explicit_bodies.len() > 1 {
        let combined_text_len: usize = explicit_bodies
            .iter()
            .map(|candidate| candidate.features.get_count(FeatureIndex::LogTextLenChars))
            .sum();
        if best_text_len >= combined_text_len
            && best_text_len <= combined_text_len * 6 / 5
            && best_toc <= 0.1
            && best_link_density <= 0.15
        {
            return best;
        }
        let fragmented_body_parent = candidates
            .iter()
            .filter(|candidate| {
                let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
                candidate.node_id != best.node_id
                    && explicit_bodies
                        .iter()
                        .all(|body| is_descendant(arena, body.node_id, candidate.node_id))
                    && text_len >= combined_text_len
                    && text_len <= combined_text_len * 6 / 5
                    && candidate.features.get(FeatureIndex::TocLike) <= 0.1
                    && candidate.features.get(FeatureIndex::LinkDensity) <= 0.15
            })
            .max_by_key(|candidate| arena.ancestors(candidate.node_id).count());
        if let Some(candidate) = fragmented_body_parent {
            return candidate;
        }
    }
    if let Some(candidate) = explicit_body {
        return candidate;
    }

    if best_toc < 0.2 && best_cluster > 0.4 {
        return best;
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

        if !is_content_container(candidate.tag) {
            continue;
        }

        let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
        if text_len < options.min_text_chars * 2 {
            continue;
        }

        let has_min_paragraphs = candidate.features.get(FeatureIndex::HasMinParagraphs) > 0.5;
        if !has_min_paragraphs {
            continue;
        }

        let ratio = text_len as f32 / best_text_len.max(1) as f32;
        let min_ratio = if allow_smaller { 0.15 } else { 0.2 };
        if ratio < min_ratio || ratio > 0.6 {
            continue;
        }

        let cluster = candidate.features.get(FeatureIndex::ContentClusterScore);
        if cluster < 0.8 {
            continue;
        }

        let toc_like = candidate.features.get(FeatureIndex::TocLike);
        if toc_like > best_toc {
            continue;
        }

        let clean_ratio = candidate.features.get(FeatureIndex::CleanTextRatio);
        if ratio < 0.2 && (cluster < 0.9 || clean_ratio < best_clean_ratio + 0.03) {
            continue;
        }

        let avg_para_len_strict = candidate.features.get(FeatureIndex::AvgParagraphLenStrict);
        let long_para_ratio = candidate.features.get(FeatureIndex::LongParagraphRatio);
        let focus_score = cluster * 2.0 + clean_ratio - toc_like - ratio * 0.2
            + avg_para_len_strict * 0.3
            + long_para_ratio * 0.2;

        if focus_score > refined_score {
            refined = Some(candidate);
            refined_score = focus_score;
        }
    }

    if best_toc < 0.15 && best_link_density < 0.1 && best_clean_ratio > 0.9 && best_p_count >= 8 {
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

    let best_text_len = best.features.get_count(FeatureIndex::LogTextLenChars);
    if best_text_len < options.min_text_chars * 3 {
        return best;
    }

    let best_toc = best.features.get(FeatureIndex::TocLike);
    let best_link_density = best.features.get(FeatureIndex::LinkDensity);
    let best_clean_ratio = best.features.get(FeatureIndex::CleanTextRatio);
    if best_toc < 0.15 && best_link_density < 0.1 && best_clean_ratio > 0.9 {
        return best;
    }

    let best_li_count = best.features.get_count(FeatureIndex::LogLiCount);
    if best_li_count < 6 {
        return best;
    }

    let mut scored_children: Vec<(&Candidate, f32, f32)> = Vec::new();

    for candidate in candidates {
        if candidate.node_id == best.node_id {
            continue;
        }

        if !is_refinement_container(candidate.tag) {
            continue;
        }

        if arena.get(candidate.node_id).and_then(|n| n.parent) != Some(best.node_id) {
            continue;
        }

        let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
        if text_len < options.min_text_chars * 2 {
            continue;
        }

        let ratio = text_len as f32 / best_text_len.max(1) as f32;
        if !(0.25..=0.8).contains(&ratio) {
            continue;
        }

        let has_min_paragraphs = candidate.features.get(FeatureIndex::HasMinParagraphs) > 0.5;
        if !has_min_paragraphs {
            continue;
        }

        let avg_para_len_strict = candidate.features.get(FeatureIndex::AvgParagraphLenStrict);
        let long_para_ratio = candidate.features.get(FeatureIndex::LongParagraphRatio);
        let clean_ratio = candidate.features.get(FeatureIndex::CleanTextRatio);
        let toc_like = candidate.features.get(FeatureIndex::TocLike);
        let cluster = candidate.features.get(FeatureIndex::ContentClusterScore);

        let score = avg_para_len_strict * 0.8 + long_para_ratio * 0.4 + clean_ratio + cluster
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

    let p_count = best.features.get_count(FeatureIndex::LogPCount);
    if p_count > 1 {
        return best;
    }

    let parent_id = match arena.get(best.node_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return best,
    };

    let class_id = arena
        .get_attributes(parent_id)
        .map(|attrs| attrs.normalized_class_id())
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
    let should_fallback = is_small_content_tag(best.tag);

    if !should_fallback {
        return Some(best);
    }

    // Get text length
    let best_text_len = best.features.get_count(FeatureIndex::LogTextLenChars);

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
                let parent_text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);

                // Parent should have substantially more text
                if parent_text_len > best_text_len * 3 {
                    // Check parent tag is suitable
                    if matches!(
                        candidate.tag,
                        TagId::Div | TagId::Section | TagId::Article | TagId::Main
                    ) {
                        // Use tie-break score that favors more paragraphs
                        let p_count = candidate.features.get(FeatureIndex::LogPCount);
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
    arena.ancestors(node_id).any(|aid| {
        arena
            .get(aid)
            .and_then(|n| n.tag())
            .is_some_and(|t| t.is_boilerplate())
    })
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
    fn test_small_candidate_falls_back_to_substantive_semantic_article() {
        let mut summary = make_candidate(1, TagId::Div, 3.0);
        summary.features.values[FeatureIndex::LogTextLenChars as usize] = (806.0_f32).ln();
        summary.features.values[FeatureIndex::LogPCount as usize] = 6.0_f32.ln();

        let mut article = make_candidate(2, TagId::Article, 2.4);
        article.features.values[FeatureIndex::LogTextLenChars as usize] = (6_001.0_f32).ln();
        article.features.values[FeatureIndex::LogPCount as usize] = 18.0_f32.ln();
        article.features.values[FeatureIndex::SemanticMainFlag as usize] = 1.0;
        article.features.values[FeatureIndex::TocLike as usize] = 0.3;
        article.features.values[FeatureIndex::LinkDensity as usize] = 0.4;

        let candidates = vec![summary, article];
        let arena = crate::dom::Arena::new();
        let best = select_best(&candidates, &arena, &ExtractOptions::default());

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

    #[test]
    fn test_refines_broad_container_to_focused_article() {
        use crate::{dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let broad_id = arena.add_node(Node::element(TagId::Div, None));
        let article_id = arena.add_node(Node::element(TagId::Article, None));
        arena.append_child(root, broad_id);
        arena.append_child(broad_id, article_id);

        let mut broad = make_candidate(broad_id, TagId::Div, 10.0);
        broad.features = FeatureVector::default();
        broad.features.values[FeatureIndex::LogTextLenChars as usize] = 15_001.0_f32.ln();
        broad.features.values[FeatureIndex::TocLike as usize] = 0.17;
        broad.features.values[FeatureIndex::CleanTextRatio as usize] = 0.87;
        broad.features.values[FeatureIndex::ContentClusterScore as usize] = 0.2;

        let mut article = make_candidate(article_id, TagId::Article, 5.0);
        article.features = FeatureVector::default();
        article.features.values[FeatureIndex::LogTextLenChars as usize] = 5_501.0_f32.ln();
        article.features.values[FeatureIndex::TocLike as usize] = 0.1;
        article.features.values[FeatureIndex::CleanTextRatio as usize] = 0.85;
        article.features.values[FeatureIndex::ContentClusterScore as usize] = 0.5;

        let candidates = vec![broad, article];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();
        assert_eq!(best.node_id, article_id);
    }

    #[test]
    fn test_refines_toc_heavy_main_to_clean_semantic_article() {
        use crate::{dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let main_id = arena.add_node(Node::element(TagId::Main, None));
        let article_id = arena.add_node(Node::element(TagId::Article, None));
        arena.append_child(root, main_id);
        arena.append_child(main_id, article_id);

        let mut main = make_candidate(main_id, TagId::Main, 10.0);
        main.features = FeatureVector::default();
        main.features.values[FeatureIndex::LogTextLenChars as usize] = 5_001.0_f32.ln();
        main.features.values[FeatureIndex::TocLike as usize] = 0.51;
        main.features.values[FeatureIndex::CleanTextRatio as usize] = 0.64;
        main.features.values[FeatureIndex::ContentClusterScore as usize] = 0.2;

        let mut article = make_candidate(article_id, TagId::Article, 5.0);
        article.features = FeatureVector::default();
        article.features.values[FeatureIndex::LogTextLenChars as usize] = 1_426.0_f32.ln();
        article.features.values[FeatureIndex::TocLike as usize] = 0.26;
        article.features.values[FeatureIndex::CleanTextRatio as usize] = 0.8;
        article.features.values[FeatureIndex::ContentClusterScore as usize] = 0.2;
        article.features.values[FeatureIndex::SemanticMainFlag as usize] = 1.0;
        article.features.values[FeatureIndex::LogPosKwHits as usize] = 2.0_f32.ln();

        let candidates = vec![main, article];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, article_id);
    }

    #[test]
    fn test_refines_bloated_main_to_its_only_direct_article() {
        use crate::{dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let main_id = arena.add_node(Node::element(TagId::Main, None));
        let article_id = arena.add_node(Node::element(TagId::Article, None));
        arena.append_child(root, main_id);
        arena.append_child(main_id, article_id);

        let mut main = make_candidate(main_id, TagId::Main, 10.0);
        main.features = FeatureVector::default();
        main.features.values[FeatureIndex::LogTextLenChars as usize] = 100_001.0_f32.ln();
        main.features.values[FeatureIndex::CleanTextRatio as usize] = 0.2;

        let mut article = make_candidate(article_id, TagId::Article, 5.0);
        article.features = FeatureVector::default();
        article.features.values[FeatureIndex::LogTextLenChars as usize] = 4_001.0_f32.ln();
        article.features.values[FeatureIndex::LogPCount as usize] = 8.0_f32.ln();
        article.features.values[FeatureIndex::TocLike as usize] = 0.05;
        article.features.values[FeatureIndex::LinkDensity as usize] = 0.1;

        let candidates = vec![main, article];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, article_id);
    }

    #[test]
    fn test_refines_main_to_sole_focused_semantic_section() {
        use crate::{dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let main_id = arena.add_node(Node::element(TagId::Main, None));
        let section_id = arena.add_node(Node::element(TagId::Section, None));
        arena.append_child(root, main_id);
        arena.append_child(main_id, section_id);

        let mut main = make_candidate(main_id, TagId::Main, 10.0);
        main.features = FeatureVector::default();
        main.features.values[FeatureIndex::LogTextLenChars as usize] = 4_501.0_f32.ln();
        main.features.values[FeatureIndex::TocLike as usize] = 0.05;
        main.features.values[FeatureIndex::LinkDensity as usize] = 0.03;

        let mut section = make_candidate(section_id, TagId::Section, 5.0);
        section.features = FeatureVector::default();
        section.features.values[FeatureIndex::LogTextLenChars as usize] = 3_501.0_f32.ln();
        section.features.values[FeatureIndex::SemanticMainFlag as usize] = 1.0;
        section.features.values[FeatureIndex::LogPosKwHits as usize] = 2.0_f32.ln();
        section.features.values[FeatureIndex::TocLike as usize] = 0.06;
        section.features.values[FeatureIndex::LinkDensity as usize] = 0.04;

        let candidates = vec![main, section];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, section_id);
    }

    #[test]
    fn test_refines_to_explicit_clean_article_body() {
        use crate::{dom::Attributes, dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let section_id = arena.add_node(Node::element(TagId::Section, None));
        let content_id = arena.add_node(Node::element(TagId::Div, None));
        arena.append_child(root, section_id);
        arena.append_child(section_id, content_id);
        arena.set_attributes(
            content_id,
            Attributes {
                class: Some("articleBody".to_string()),
                itemprop: Some("articleBody".to_string()),
                ..Default::default()
            },
        );

        let mut section = make_candidate(section_id, TagId::Section, 10.0);
        section.features = FeatureVector::default();
        section.features.values[FeatureIndex::LogTextLenChars as usize] = 4_501.0_f32.ln();
        section.features.values[FeatureIndex::TocLike as usize] = 0.15;
        section.features.values[FeatureIndex::CleanTextRatio as usize] = 0.86;
        section.features.values[FeatureIndex::ContentClusterScore as usize] = 1.0;

        let mut content = make_candidate(content_id, TagId::Div, 5.0);
        content.features = FeatureVector::default();
        content.features.values[FeatureIndex::LogTextLenChars as usize] = 851.0_f32.ln();
        content.features.values[FeatureIndex::TocLike as usize] = 0.01;
        content.features.values[FeatureIndex::CleanTextRatio as usize] = 0.92;
        content.features.values[FeatureIndex::HasMinParagraphs as usize] = 1.0;

        let candidates = vec![section, content];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, content_id);
    }

    #[test]
    fn test_refines_to_substantive_direct_article_body() {
        use crate::{dom::Attributes, dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let broad_id = arena.add_node(Node::element(TagId::Div, None));
        let content_id = arena.add_node(Node::element(TagId::Div, None));
        arena.append_child(root, broad_id);
        arena.append_child(broad_id, content_id);
        arena.set_attributes(
            content_id,
            Attributes {
                class: Some("articleBody".to_string()),
                ..Default::default()
            },
        );

        let mut broad = make_candidate(broad_id, TagId::Div, 10.0);
        broad.features = FeatureVector::default();
        broad.features.values[FeatureIndex::LogTextLenChars as usize] = 15_090.0_f32.ln();
        broad.features.values[FeatureIndex::CleanTextRatio as usize] = 0.84;

        let mut content = make_candidate(content_id, TagId::Div, 5.0);
        content.features = FeatureVector::default();
        content.features.values[FeatureIndex::LogTextLenChars as usize] = 5_283.0_f32.ln();
        content.features.values[FeatureIndex::LogPCount as usize] = 28.0_f32.ln();
        content.features.values[FeatureIndex::CleanTextRatio as usize] = 0.887;
        content.features.values[FeatureIndex::HasMinParagraphs as usize] = 1.0;
        content.features.values[FeatureIndex::TocLike as usize] = 0.08;

        let candidates = vec![broad, content];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, content_id);
    }

    #[test]
    fn test_keeps_fragmented_explicit_article_bodies_together() {
        use crate::{dom::Attributes, dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let broad_id = arena.add_node(Node::element(TagId::Div, None));
        let chunks_id = arena.add_node(Node::element(TagId::Div, None));
        let first_id = arena.add_node(Node::element(TagId::Div, None));
        let second_id = arena.add_node(Node::element(TagId::Div, None));
        arena.append_child(root, broad_id);
        arena.append_child(broad_id, chunks_id);
        arena.append_child(chunks_id, first_id);
        arena.append_child(chunks_id, second_id);
        for body_id in [first_id, second_id] {
            arena.set_attributes(
                body_id,
                Attributes {
                    class: Some("article__body".to_string()),
                    ..Default::default()
                },
            );
        }

        let mut broad = make_candidate(broad_id, TagId::Div, 10.0);
        broad.features = FeatureVector::default();
        broad.features.values[FeatureIndex::LogTextLenChars as usize] = 10_001.0_f32.ln();
        broad.features.values[FeatureIndex::TocLike as usize] = 0.2;
        broad.features.values[FeatureIndex::CleanTextRatio as usize] = 0.7;

        let mut chunks = make_candidate(chunks_id, TagId::Div, 4.0);
        chunks.features = FeatureVector::default();
        chunks.features.values[FeatureIndex::LogTextLenChars as usize] = 7_001.0_f32.ln();
        chunks.features.values[FeatureIndex::TocLike as usize] = 0.02;
        chunks.features.values[FeatureIndex::LinkDensity as usize] = 0.05;

        let mut first = make_candidate(first_id, TagId::Div, 5.0);
        first.features = FeatureVector::default();
        first.features.values[FeatureIndex::LogTextLenChars as usize] = 3_501.0_f32.ln();
        first.features.values[FeatureIndex::CleanTextRatio as usize] = 0.95;
        first.features.values[FeatureIndex::HasMinParagraphs as usize] = 1.0;

        let mut second = make_candidate(second_id, TagId::Div, 5.0);
        second.features = first.features.clone();

        let candidates = vec![broad, chunks, first, second];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();

        assert_eq!(best.node_id, chunks_id);
    }

    #[test]
    fn test_refines_article_to_direct_content_container() {
        use crate::{dom::Node, features::FeatureVector};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let article_id = arena.add_node(Node::element(TagId::Article, None));
        let content_id = arena.add_node(Node::element(TagId::Div, None));
        arena.append_child(root, article_id);
        arena.append_child(article_id, content_id);

        let mut article = make_candidate(article_id, TagId::Article, 10.0);
        article.features = FeatureVector::default();
        article.features.values[FeatureIndex::LogTextLenChars as usize] = 5_501.0_f32.ln();
        article.features.values[FeatureIndex::TocLike as usize] = 0.1;

        let mut content = make_candidate(content_id, TagId::Div, 5.0);
        content.features = FeatureVector::default();
        content.features.values[FeatureIndex::LogTextLenChars as usize] = 5_301.0_f32.ln();
        content.features.values[FeatureIndex::TocLike as usize] = 0.05;
        content.features.values[FeatureIndex::SemanticMainFlag as usize] = 1.0;
        content.features.values[FeatureIndex::LogPosKwHits as usize] = 2.0_f32.ln();

        let candidates = vec![article, content];
        let best = select_best(&candidates, &arena, &ExtractOptions::default()).unwrap();
        assert_eq!(best.node_id, content_id);
    }
}
