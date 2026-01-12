//! Paragraph statistics and content diversity analysis

use std::collections::HashSet;

use crate::dom::{Arena, NodeId, NodeKind, TagId};

/// Paragraph statistics for anti-over-extraction features
pub struct ParagraphStats {
    pub count: usize,
    pub avg_len: f32,
    pub long_count: usize,
    pub substantial_count: usize,
    pub cluster_score: f32,
}

/// Compute paragraph statistics for a subtree
pub fn compute_paragraph_stats(arena: &Arena, node_id: NodeId) -> ParagraphStats {
    let mut paragraphs: Vec<(NodeId, usize)> = Vec::new();

    // Find all <p> elements and their text lengths
    for desc_id in arena.descendants(node_id) {
        if let Some(node) = arena.get(desc_id) {
            if let NodeKind::Element { tag: TagId::P, .. } = &node.kind {
                let text_len = arena.collect_text(desc_id).chars().count();
                paragraphs.push((desc_id, text_len));
            }
        }
    }

    let count = paragraphs.len();
    if count == 0 {
        return ParagraphStats {
            count: 0,
            avg_len: 0.0,
            long_count: 0,
            substantial_count: 0,
            cluster_score: 0.0,
        };
    }

    let total_len: usize = paragraphs.iter().map(|(_, len)| len).sum();
    let avg_len = total_len as f32 / count as f32;

    let long_count = paragraphs.iter().filter(|(_, len)| *len > 80).count();
    let substantial_count = paragraphs.iter().filter(|(_, len)| *len > 50).count();

    // Compute cluster score: are paragraphs close together or scattered?
    // Higher score = more clustered (good for content)
    let cluster_score = if count >= 2 {
        // Get depths of paragraphs
        let depths: Vec<u32> = paragraphs
            .iter()
            .filter_map(|(id, _)| arena.get(*id).map(|n| n.depth))
            .collect();

        if depths.is_empty() {
            0.0
        } else {
            // Check if paragraphs are at similar depths
            let min_depth = *depths.iter().min().unwrap_or(&0);
            let max_depth = *depths.iter().max().unwrap_or(&0);
            let depth_variance = max_depth - min_depth;

            // Low variance = high cluster score
            if depth_variance <= 2 {
                1.0
            } else if depth_variance <= 4 {
                0.5
            } else {
                0.2
            }
        }
    } else {
        0.5 // Single paragraph - neutral score
    };

    ParagraphStats {
        count,
        avg_len,
        long_count,
        substantial_count,
        cluster_score,
    }
}

/// Compute descendant diversity: variety of content-related tags in subtree
/// Higher diversity often indicates real content rather than navigation/boilerplate
pub fn compute_descendant_diversity(arena: &Arena, node_id: NodeId) -> f32 {
    // Content-related tags to track
    let content_tags = [
        TagId::P,
        TagId::H1,
        TagId::H2,
        TagId::H3,
        TagId::H4,
        TagId::H5,
        TagId::H6,
        TagId::Pre,
        TagId::Code,
        TagId::Blockquote,
        TagId::Ul,
        TagId::Ol,
        TagId::Li,
        TagId::Table,
        TagId::Figure,
        TagId::Img,
    ];

    let mut found_tags: HashSet<TagId> = HashSet::new();

    for desc_id in arena.descendants(node_id) {
        if let Some(node) = arena.get(desc_id) {
            if let NodeKind::Element { tag, .. } = &node.kind {
                if content_tags.contains(tag) {
                    found_tags.insert(*tag);
                }
            }
        }
    }

    // Normalize: 0 tags = 0.0, 5+ tags = 1.0
    (found_tags.len() as f32 / 5.0).min(1.0)
}
