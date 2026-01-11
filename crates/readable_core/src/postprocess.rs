//! Post-processing improvements inspired by Readability.js
//!
//! This module implements:
//! 1. Score propagation from children to ancestors
//! 2. Sibling expansion for fragmented content
//! 3. Boilerplate removal from selected content

use std::collections::HashMap;

use crate::{
    candidates::{negative_keywords, Candidate},
    dom::{Arena, NodeId, NodeKind, TagId},
    features::{extract_features, FeatureIndex},
};

struct SelectedSiblingMetrics {
    avg_para_len_strict: f32,
    long_para_ratio: f32,
    li_count: usize,
    p_count: usize,
}

/// Apply score propagation to candidates
///
/// Readability.js propagates scores from content elements to their ancestors:
/// - Parent gets score / 1
/// - Grandparent gets score / 2
/// - Great-grandparent gets score / 3
/// etc.
///
/// This helps containers with multiple good paragraphs "bubble up" their quality.
pub fn apply_score_propagation(candidates: &mut [Candidate], arena: &Arena) {
    // Build a map from node_id to candidate index for quick lookup
    let mut node_to_idx: HashMap<NodeId, usize> = HashMap::new();
    for (idx, candidate) in candidates.iter().enumerate() {
        node_to_idx.insert(candidate.node_id, idx);
    }

    // Collect propagation deltas (to avoid borrowing issues)
    let mut deltas: HashMap<NodeId, f32> = HashMap::new();

    // For each candidate, propagate score to ancestors
    for candidate in candidates.iter() {
        // Only propagate positive scores
        if candidate.score <= 0.0 {
            continue;
        }

        // Skip very small candidates (not enough signal)
        let text_len = (candidate.features.values[FeatureIndex::LogTextLenChars as usize].exp()
            - 1.0) as usize;
        if text_len < 50 {
            continue;
        }

        // Propagate to ancestors with diminishing weight
        let mut depth = 0;
        let mut current = arena.get(candidate.node_id).and_then(|n| n.parent);

        while let Some(parent_id) = current {
            depth += 1;

            // Diminishing divisor (1, 2, 3, ...)
            let divisor = depth as f32;

            // Propagation amount based on Readability.js formula
            // Score contribution diminishes with depth
            let propagated = candidate.score / divisor;

            // Only propagate if ancestor is also a candidate
            if node_to_idx.contains_key(&parent_id) {
                *deltas.entry(parent_id).or_insert(0.0) += propagated;
            }

            // Stop after 3 levels (parent, grandparent, great-grandparent)
            if depth >= 3 {
                break;
            }

            current = arena.get(parent_id).and_then(|n| n.parent);
        }
    }

    // Apply the accumulated deltas
    for candidate in candidates.iter_mut() {
        if let Some(delta) = deltas.get(&candidate.node_id) {
            candidate.score += delta;
        }
    }
}

/// Check if a sibling should be included based on content similarity
fn should_include_sibling(
    arena: &Arena,
    sibling_id: NodeId,
    best_text_len: usize,
    selected_metrics: &SelectedSiblingMetrics,
) -> bool {
    let sibling = match arena.get(sibling_id) {
        Some(n) => n,
        None => return false,
    };

    // Only consider element nodes
    let tag = match &sibling.kind {
        NodeKind::Element { tag, .. } => *tag,
        _ => return false,
    };

    // Skip obvious boilerplate tags
    if tag.is_boilerplate() {
        return false;
    }

    // Get sibling text
    let sibling_text = arena.collect_text(sibling_id);
    let sibling_text_len = sibling_text.chars().count();

    // Sibling should have substantial text (at least 20% of best)
    if sibling_text_len < best_text_len / 5 {
        return false;
    }

    // Check link density - should be low for content
    let link_text_len = compute_link_text_len(arena, sibling_id);
    let link_density = link_text_len as f32 / sibling_text_len.max(1) as f32;

    if link_density > 0.3 {
        return false;
    }

    // Check for negative keywords in class/id
    if let Some(attrs) = arena.get_attributes(sibling_id) {
        let class_id = format!(
            "{} {}",
            attrs.class.as_deref().unwrap_or(""),
            attrs.id.as_deref().unwrap_or("")
        )
        .to_lowercase();

        let negative =
            ["sidebar", "nav", "footer", "header", "comment", "ad", "promo", "related", "social"];
        for kw in negative {
            if class_id.contains(kw) {
                return false;
            }
        }
    }

    // Avoid pulling in teaser-heavy sibling lists when the selected content
    // has meaningfully longer paragraphs.
    let list_heavy = selected_metrics.li_count >= 5
        && selected_metrics.li_count.saturating_mul(2) >= selected_metrics.p_count;
    if list_heavy && selected_metrics.avg_para_len_strict > 0.25 {
        let sibling_features = extract_features(arena, sibling_id);
        let sibling_avg_para_len_strict =
            sibling_features.values[FeatureIndex::AvgParagraphLenStrict as usize];
        let sibling_long_para_ratio =
            sibling_features.values[FeatureIndex::LongParagraphRatio as usize];

        let min_avg = selected_metrics.avg_para_len_strict * 0.8;
        let min_long_ratio = (selected_metrics.long_para_ratio - 0.05).max(0.0);
        if sibling_avg_para_len_strict < min_avg || sibling_long_para_ratio < min_long_ratio {
            return false;
        }
    }

    // Prefer semantic/content siblings
    if matches!(
        tag,
        TagId::Article | TagId::Section | TagId::Div | TagId::P | TagId::Td | TagId::Tr
    ) {
        return true;
    }

    // For other tags, require higher text threshold
    sibling_text_len >= best_text_len / 3
}

#[derive(Clone, Copy)]
enum PassThrough {
    Include,
    Skip,
}

fn is_media_tag(tag: TagId) -> bool {
    matches!(tag, TagId::Img | TagId::Figure | TagId::Figcaption | TagId::Video | TagId::Audio)
}

fn has_media_descendant(arena: &Arena, node_id: NodeId) -> bool {
    for desc_id in arena.descendants(node_id) {
        if let Some(node) = arena.get(desc_id) {
            if let NodeKind::Element { tag, .. } = &node.kind {
                if is_media_tag(*tag) {
                    return true;
                }
            }
        }
    }
    false
}

fn pass_through_sibling(
    arena: &Arena,
    sibling_id: NodeId,
    best_text_len: usize,
) -> Option<PassThrough> {
    let sibling = arena.get(sibling_id)?;
    let tag = match &sibling.kind {
        NodeKind::Element { tag, .. } => *tag,
        _ => return None,
    };

    let sibling_text_len = arena.collect_text(sibling_id).chars().count();

    if is_media_tag(tag) {
        return Some(PassThrough::Include);
    }

    if sibling_text_len < best_text_len / 6 && has_media_descendant(arena, sibling_id) {
        return Some(PassThrough::Include);
    }

    if sibling_text_len == 0 && matches!(tag, TagId::Div | TagId::Span) {
        return Some(PassThrough::Skip);
    }

    if tag.is_boilerplate() {
        if sibling_text_len < best_text_len / 3 {
            return Some(PassThrough::Skip);
        }
        return None;
    }

    if let Some(attrs) = arena.get_attributes(sibling_id) {
        if attrs.contains_keyword(negative_keywords()) {
            if sibling_text_len < best_text_len / 3 {
                return Some(PassThrough::Skip);
            }
            return None;
        }
    }

    let link_text_len = compute_link_text_len(arena, sibling_id);
    let link_density = link_text_len as f32 / sibling_text_len.max(1) as f32;

    if sibling_text_len < best_text_len / 4 && link_density > 0.6 {
        return Some(PassThrough::Skip);
    }

    None
}

/// Compute link text length in a subtree
fn compute_link_text_len(arena: &Arena, node_id: NodeId) -> usize {
    let mut total = 0;
    for desc_id in arena.descendants(node_id) {
        if let Some(node) = arena.get(desc_id) {
            if let NodeKind::Element { tag: TagId::A, .. } = &node.kind {
                total += arena.collect_text(desc_id).chars().count();
            }
        }
    }
    total
}

/// Find siblings that should be included with the selected content
///
/// This helps with fragmented content where the main article is split
/// across multiple sibling elements.
pub fn find_expansion_siblings(
    arena: &Arena,
    selected_id: NodeId,
    selected_text_len: usize,
) -> Vec<NodeId> {
    let mut siblings = Vec::new();
    let selected_features = extract_features(arena, selected_id);
    let selected_metrics = SelectedSiblingMetrics {
        avg_para_len_strict: selected_features.values[FeatureIndex::AvgParagraphLenStrict as usize],
        long_para_ratio: selected_features.values[FeatureIndex::LongParagraphRatio as usize],
        li_count: (selected_features.values[FeatureIndex::LogLiCount as usize].exp() - 1.0)
            as usize,
        p_count: (selected_features.values[FeatureIndex::LogPCount as usize].exp() - 1.0)
            as usize,
    };

    // Get the parent
    let _parent_id = match arena.get(selected_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return siblings,
    };

    // Check preceding siblings
    let mut skipped = 0usize;
    let mut prev = selected_id;
    while let Some(prev_id) = arena.get(prev).and_then(|n| n.prev_sibling) {
        if should_include_sibling(arena, prev_id, selected_text_len, &selected_metrics) {
            siblings.push(prev_id);
            skipped = 0;
        } else if let Some(pass_through) = pass_through_sibling(arena, prev_id, selected_text_len) {
            if matches!(pass_through, PassThrough::Include) {
                siblings.push(prev_id);
            }
            skipped += 1;
            if skipped > 2 {
                break;
            }
        } else {
            // Stop at first non-matching sibling to maintain contiguity
            break;
        }
        prev = prev_id;
    }

    // Reverse to maintain document order
    siblings.reverse();

    // Check following siblings
    let mut skipped = 0usize;
    let mut next = selected_id;
    while let Some(next_id) = arena.get(next).and_then(|n| n.next_sibling) {
        if should_include_sibling(arena, next_id, selected_text_len, &selected_metrics) {
            siblings.push(next_id);
            skipped = 0;
        } else if let Some(pass_through) = pass_through_sibling(arena, next_id, selected_text_len) {
            if matches!(pass_through, PassThrough::Include) {
                siblings.push(next_id);
            }
            skipped += 1;
            if skipped > 2 {
                break;
            }
        } else {
            break;
        }
        next = next_id;
    }

    siblings
}

/// Nodes within the selected content that should be removed as boilerplate
pub fn find_boilerplate_descendants(arena: &Arena, root_id: NodeId) -> Vec<NodeId> {
    let mut boilerplate = Vec::new();

    for desc_id in arena.descendants(root_id) {
        if is_boilerplate_element(arena, desc_id) {
            boilerplate.push(desc_id);
        }
    }

    boilerplate
}

/// Public function for cleanup module to check if element is boilerplate
pub fn is_boilerplate_for_cleanup(arena: &Arena, node_id: NodeId) -> bool {
    is_boilerplate_element(arena, node_id)
}

/// Check if a descendant element should be removed as boilerplate
/// This is more conservative than the original to avoid false positives
fn is_boilerplate_element(arena: &Arena, node_id: NodeId) -> bool {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return false,
    };

    let tag = match &node.kind {
        NodeKind::Element { tag, .. } => *tag,
        _ => return false,
    };

    // Only check nav/aside tags with very high confidence
    if matches!(tag, TagId::Nav | TagId::Aside) {
        let text = arena.collect_text(node_id);
        let text_len = text.chars().count();

        // Very small nav/aside elements are likely boilerplate
        if text_len < 100 {
            return true;
        }

        // Check link density
        let link_text_len = compute_link_text_len(arena, node_id);
        let link_density = link_text_len as f32 / text_len.max(1) as f32;

        // Very high link density = definitely navigation
        return link_density > 0.6;
    }

    // Check class/id for strong boilerplate signals only
    if let Some(attrs) = arena.get_attributes(node_id) {
        let class_id = format!(
            "{} {}",
            attrs.class.as_deref().unwrap_or(""),
            attrs.id.as_deref().unwrap_or("")
        )
        .to_lowercase();

        // Only very strong negative signals with small content
        let strong_negative = ["advertisement", "ad-container", "promo-box"];
        for kw in strong_negative {
            if class_id.contains(kw) {
                let text_len = arena.collect_text(node_id).chars().count();
                if text_len < 200 {
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
    #[cfg(test)]
    use crate::candidates::generate_candidates;
    #[cfg(test)]
    use crate::features::extract_features;
    #[cfg(test)]
    use crate::model::score;
    use crate::parse::parse_html;

    #[test]
    fn test_score_propagation() {
        let html = r#"
        <html><body>
            <article id="main">
                <p>First paragraph with some content.</p>
                <p>Second paragraph with more content.</p>
            </article>
        </body></html>
        "#;

        let arena = parse_html(html);
        let mut candidates = generate_candidates(&arena, 100);

        // Compute features and scores
        for c in &mut candidates {
            c.features = extract_features(&arena, c.node_id);
            c.score = score(&c.features);
        }

        // Find article candidate
        let article_score_before = candidates
            .iter()
            .find(|c| c.tag == TagId::Article)
            .map(|c| c.score)
            .unwrap_or(0.0);

        // Apply propagation
        apply_score_propagation(&mut candidates, &arena);

        // Article should have higher score after propagation from paragraphs
        let article_score_after = candidates
            .iter()
            .find(|c| c.tag == TagId::Article)
            .map(|c| c.score)
            .unwrap_or(0.0);

        assert!(
            article_score_after >= article_score_before,
            "Score propagation should increase parent scores"
        );
    }

    #[test]
    fn test_find_boilerplate() {
        let html = r##"
        <html><body>
            <article>
                <p>Main content here.</p>
                <nav class="article-nav">
                    <a href="#">Link 1</a>
                    <a href="#">Link 2</a>
                </nav>
                <aside class="sidebar">Related links</aside>
            </article>
        </body></html>
        "##;

        let arena = parse_html(html);
        let article_id = arena
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| matches!(&n.kind, NodeKind::Element { tag: TagId::Article, .. }))
            .map(|(i, _)| i as NodeId)
            .unwrap();

        let boilerplate = find_boilerplate_descendants(&arena, article_id);

        // Should find nav and aside as boilerplate
        assert!(!boilerplate.is_empty());
    }
}
