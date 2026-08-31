//! Post-processing improvements inspired by Readability.js
//!
//! This module implements:
//! 1. Score propagation from children to ancestors
//! 2. Sibling expansion for fragmented content
//! 3. Boilerplate removal from selected content

use std::collections::HashMap;

use crate::{
    candidates::Candidate,
    dom::{Arena, NodeId, TagId},
    features::{FeatureIndex, extract_features},
    keywords::negative_keywords,
    tags::is_media_tag,
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
/// - Great-grandparent gets score / 3 etc.
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
        let text_len = candidate.features.get_count(FeatureIndex::LogTextLenChars);
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
    let tag = match sibling.tag() {
        Some(t) => t,
        None => return false,
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
        let class_id = attrs.normalized_class_id();

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
        let sibling_avg_para_len_strict = sibling_features.get(FeatureIndex::AvgParagraphLenStrict);
        let sibling_long_para_ratio = sibling_features.get(FeatureIndex::LongParagraphRatio);

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

fn has_media_descendant(arena: &Arena, node_id: NodeId) -> bool {
    for desc_id in arena.descendants(node_id) {
        if let Some(tag) = arena.get(desc_id).and_then(|n| n.tag())
            && is_media_tag(tag)
        {
            return true;
        }
    }
    false
}

fn is_text_media_module_pair(arena: &Arena, text_id: NodeId, media_id: NodeId) -> bool {
    let class = |node_id| arena.get_attributes(node_id).and_then(|attrs| attrs.class.as_deref());
    let (Some(text_class), Some(media_class)) = (class(text_id), class(media_id)) else {
        return false;
    };
    let shared_module = match (
        text_class.split_ascii_whitespace().next(),
        media_class.split_ascii_whitespace().next(),
    ) {
        (Some(first), Some(second)) => first.len() >= 5 && first == second,
        _ => false,
    };
    shared_module
        && text_class.contains("text")
        && (media_class.contains("image") || media_class.contains("media"))
}

fn pass_through_sibling(
    arena: &Arena,
    selected_id: NodeId,
    sibling_id: NodeId,
    best_text_len: usize,
) -> Option<PassThrough> {
    let sibling = arena.get(sibling_id)?;
    let tag = sibling.tag()?;

    let sibling_text_len = arena.collect_text(sibling_id).chars().count();

    if is_media_tag(tag) {
        return Some(PassThrough::Include);
    }

    let expanded_media_limit = is_text_media_module_pair(arena, selected_id, sibling_id);
    let media_text_limit = if expanded_media_limit {
        best_text_len / 4
    } else {
        best_text_len / 6
    };
    if sibling_text_len < media_text_limit && has_media_descendant(arena, sibling_id) {
        if !expanded_media_limit {
            return Some(PassThrough::Include);
        }
        let link_text_len = compute_link_text_len(arena, sibling_id);
        let link_density = link_text_len as f32 / sibling_text_len.max(1) as f32;
        if link_density <= 0.2 {
            return Some(PassThrough::Include);
        }
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

    if let Some(attrs) = arena.get_attributes(sibling_id)
        && attrs.contains_keyword(negative_keywords())
    {
        if sibling_text_len < best_text_len / 3 {
            return Some(PassThrough::Skip);
        }
        return None;
    }

    let link_text_len = compute_link_text_len(arena, sibling_id);
    let link_density = link_text_len as f32 / sibling_text_len.max(1) as f32;

    if sibling_text_len < best_text_len / 4 && link_density > 0.6 {
        return Some(PassThrough::Skip);
    }

    None
}

fn starts_with_horizontal_rule(arena: &Arena, node_id: NodeId) -> bool {
    for child_id in arena.children(node_id) {
        let Some(child) = arena.get(child_id) else {
            continue;
        };
        match &child.kind {
            crate::dom::NodeKind::Text => {
                if arena
                    .get_text(child_id)
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return false;
                }
            }
            crate::dom::NodeKind::Element { tag, tag_name } => {
                if *tag == TagId::Other
                    && tag_name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case("hr"))
                {
                    return true;
                }
                if matches!(tag, TagId::Div | TagId::Section | TagId::Span) {
                    return starts_with_horizontal_rule(arena, child_id);
                }
                return false;
            }
            crate::dom::NodeKind::Document => return starts_with_horizontal_rule(arena, child_id),
            crate::dom::NodeKind::Comment => {}
        }
    }
    false
}

fn is_trailing_postscript(
    arena: &Arena,
    sibling_id: NodeId,
    best_text_len: usize,
    selected_metrics: &SelectedSiblingMetrics,
) -> bool {
    if selected_metrics.p_count < 5 || !starts_with_horizontal_rule(arena, sibling_id) {
        return false;
    }

    let sibling_features = extract_features(arena, sibling_id);
    let sibling_text_len = sibling_features.get_count(FeatureIndex::LogTextLenChars);
    let sibling_p_count = sibling_features.get_count(FeatureIndex::LogPCount);
    sibling_text_len < best_text_len / 2 && sibling_p_count <= 2
}

/// Compute link text length in a subtree
pub fn compute_link_text_len(arena: &Arena, node_id: NodeId) -> usize {
    let mut total = 0;
    for desc_id in arena.descendants(node_id) {
        if arena.get(desc_id).and_then(|n| n.tag()) == Some(TagId::A) {
            total += arena.collect_text(desc_id).chars().count();
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
        avg_para_len_strict: selected_features.get(FeatureIndex::AvgParagraphLenStrict),
        long_para_ratio: selected_features.get(FeatureIndex::LongParagraphRatio),
        li_count: selected_features.get_count(FeatureIndex::LogLiCount),
        p_count: selected_features.get_count(FeatureIndex::LogPCount),
    };

    // Get the parent
    let _parent_id = match arena.get(selected_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return siblings,
    };

    // Check preceding siblings
    let mut skipped = 0usize;
    for prev_id in arena.prev_siblings(selected_id) {
        if should_include_sibling(arena, prev_id, selected_text_len, &selected_metrics) {
            siblings.push(prev_id);
            skipped = 0;
        } else if let Some(pass_through) =
            pass_through_sibling(arena, selected_id, prev_id, selected_text_len)
        {
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
    }

    // Reverse to maintain document order
    siblings.reverse();

    // Check following siblings
    let mut skipped = 0usize;
    for next_id in arena.next_siblings(selected_id) {
        if is_trailing_postscript(arena, next_id, selected_text_len, &selected_metrics) {
            break;
        }
        if should_include_sibling(arena, next_id, selected_text_len, &selected_metrics) {
            siblings.push(next_id);
            skipped = 0;
        } else if let Some(pass_through) =
            pass_through_sibling(arena, selected_id, next_id, selected_text_len)
        {
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
    let tag = match arena.get(node_id).and_then(|n| n.tag()) {
        Some(t) => t,
        None => return false,
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
        let class_id = attrs.normalized_class_id();

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

// --- Highlight and step module heuristics ---

/// Find a highlight container ancestor (for pages with "highlights" sections)
pub fn find_highlight_ancestor(arena: &Arena, node_id: NodeId) -> Option<NodeId> {
    if is_highlight_container(arena, node_id) {
        return Some(node_id);
    }

    arena
        .ancestors(node_id)
        .find(|&ancestor_id| is_highlight_container(arena, ancestor_id))
}

fn is_highlight_container(arena: &Arena, node_id: NodeId) -> bool {
    let tag = match arena.get(node_id).and_then(|n| n.tag()) {
        Some(t) => t,
        None => return false,
    };
    if !matches!(tag, TagId::Section | TagId::Div) {
        return false;
    }
    arena
        .get_attributes(node_id)
        .is_some_and(|attrs| attrs.contains_keyword(&["highlight", "highlights"]))
}

/// Check if a sibling should be included with highlight content
pub fn should_include_highlight_sibling(
    arena: &Arena,
    node_id: NodeId,
    min_text_chars: usize,
) -> bool {
    let tag = arena.get(node_id).and_then(|n| n.tag());
    if !matches!(tag, Some(TagId::Section | TagId::Div | TagId::Article)) {
        return false;
    }
    let sibling_features = extract_features(arena, node_id);
    let sibling_text_len = sibling_features.get_count(FeatureIndex::LogTextLenChars);
    sibling_text_len >= min_text_chars / 2
}

/// Check if node has H2 or H3 heading descendants
pub fn has_heading_descendant(arena: &Arena, node_id: NodeId) -> bool {
    arena.has_descendant_with_tags(node_id, &[TagId::H2, TagId::H3])
}

/// Find step modules in ancestors (for step-by-step instruction pages)
pub fn find_step_modules(arena: &Arena, node_id: NodeId) -> Option<Vec<NodeId>> {
    for ancestor_id in arena.ancestors(node_id) {
        let mut step_children = Vec::new();
        for child_id in arena.children(ancestor_id) {
            if is_step_module(arena, child_id) {
                step_children.push(child_id);
            }
        }

        if step_children.len() >= 3 {
            return Some(step_children);
        }
    }

    None
}

fn is_step_module(arena: &Arena, node_id: NodeId) -> bool {
    let tag = match arena.get(node_id).and_then(|n| n.tag()) {
        Some(t) => t,
        None => return false,
    };

    if !matches!(tag, TagId::Div | TagId::Section | TagId::Article) {
        return false;
    }

    let attrs = arena.get_attributes(node_id);
    if !attrs.is_some_and(|a| a.contains_keyword(&["step"])) {
        return false;
    }

    has_step_content_descendant(arena, node_id)
}

fn has_step_content_descendant(arena: &Arena, node_id: NodeId) -> bool {
    for desc_id in arena.descendants(node_id) {
        if let Some(attrs) = arena.get_attributes(desc_id)
            && attrs.contains_keyword(&["stepcontent", "step-content"])
        {
            return true;
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
            .find(|(_, n)| n.tag() == Some(TagId::Article))
            .map(|(i, _)| i as NodeId)
            .unwrap();

        let boilerplate = find_boilerplate_descendants(&arena, article_id);

        // Should find nav and aside as boilerplate
        assert!(!boilerplate.is_empty());
    }

    #[test]
    fn test_sibling_expansion_stops_at_trailing_postscript() {
        let html = r#"
        <html><body><div>
            <div id="content">
                <p>First substantial article paragraph with enough detail for extraction.</p>
                <p>Second substantial article paragraph with enough detail for extraction.</p>
                <p>Third substantial article paragraph with enough detail for extraction.</p>
                <p>Fourth substantial article paragraph with enough detail for extraction.</p>
                <p>Fifth substantial article paragraph with enough detail for extraction.</p>
                <p>Sixth substantial article paragraph with enough detail for extraction and
                additional context that makes the primary body clearly longer than the postscript.</p>
            </div>
            <div id="postscript"><div><hr>
                <p>A shorter corporate profile follows the article after a thematic break. It
                contains enough text to satisfy the normal sibling expansion threshold, but it
                is a separate postscript rather than another fragment of the selected article.</p>
            </div></div>
        </div></body></html>
        "#;
        let arena = parse_html(html);
        let selected_id = (0..arena.nodes.len() as NodeId)
            .find(|&id| {
                arena
                    .get_attributes(id)
                    .and_then(|attrs| attrs.id.as_deref())
                    == Some("content")
            })
            .unwrap();
        let selected_text_len = arena.collect_text(selected_id).chars().count();

        assert!(find_expansion_siblings(&arena, selected_id, selected_text_len).is_empty());
    }

    #[test]
    fn test_sibling_expansion_keeps_normal_content_fragment() {
        let html = r#"
        <html><body><div>
            <div id="content">
                <p>First substantial article paragraph with enough detail for extraction.</p>
                <p>Second substantial article paragraph with enough detail for extraction.</p>
                <p>Third substantial article paragraph with enough detail for extraction.</p>
                <p>Fourth substantial article paragraph with enough detail for extraction.</p>
                <p>Fifth substantial article paragraph with enough detail for extraction.</p>
            </div>
            <div id="continuation">
                <p>A normal continuation remains eligible for sibling expansion because it does
                not begin with a horizontal rule marking a separate postscript section.</p>
            </div>
        </div></body></html>
        "#;
        let arena = parse_html(html);
        let selected_id = (0..arena.nodes.len() as NodeId)
            .find(|&id| {
                arena
                    .get_attributes(id)
                    .and_then(|attrs| attrs.id.as_deref())
                    == Some("content")
            })
            .unwrap();
        let continuation_id = (0..arena.nodes.len() as NodeId)
            .find(|&id| {
                arena
                    .get_attributes(id)
                    .and_then(|attrs| attrs.id.as_deref())
                    == Some("continuation")
            })
            .unwrap();
        let selected_text_len = arena.collect_text(selected_id).chars().count();

        assert_eq!(
            find_expansion_siblings(&arena, selected_id, selected_text_len),
            vec![continuation_id]
        );
    }

    #[test]
    fn test_sibling_expansion_crosses_captioned_media() {
        let html = format!(
            r#"
            <html><body><div>
                <div id="content" class="story-module story-text"><p>{}</p></div>
                <div id="media" class="story-module story-image"><img src="photo.jpg"><p>{}</p></div>
                <div id="continuation" class="story-module story-text"><p>{}</p></div>
            </div></body></html>
            "#,
            "Selected article text. ".repeat(50),
            "A descriptive image caption. ".repeat(8),
            "The article continues after the photograph. ".repeat(10),
        );
        let arena = parse_html(&html);
        let find_id = |expected| {
            (0..arena.nodes.len() as NodeId)
                .find(|&id| {
                    arena
                        .get_attributes(id)
                        .and_then(|attrs| attrs.id.as_deref())
                        == Some(expected)
                })
                .unwrap()
        };
        let selected_id = find_id("content");
        let media_id = find_id("media");
        let continuation_id = find_id("continuation");
        let selected_text_len = arena.collect_text(selected_id).chars().count();

        assert_eq!(
            find_expansion_siblings(&arena, selected_id, selected_text_len),
            vec![media_id, continuation_id]
        );
    }
}
