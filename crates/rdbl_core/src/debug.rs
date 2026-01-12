//! Debug output for extraction tuning
//!
//! Provides detailed information about candidate scoring for debugging.

use serde::{Deserialize, Serialize};

use crate::{
    candidates::Candidate,
    dom::{Arena, NodeId, NodeKind},
    features::{FeatureIndex, FeatureVector},
};

/// Debug information for extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugInfo {
    /// Top-K candidates with scores and features
    pub candidates: Vec<CandidateDebug>,
}

/// Debug information for a single candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDebug {
    /// Node path (tag chain with id/class snippets)
    pub node_path: String,
    /// Logistic regression logit score
    pub logit: f32,
    /// Key feature summary
    pub features: FeatureSummary,
    /// Raw 64-element feature vector for training
    pub feature_vector: Vec<f32>,
    /// Extracted text content (for training label comparison)
    pub text: String,
}

/// Summary of key features for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSummary {
    /// Text length in characters
    pub text_len: usize,
    /// Link density (0-1)
    pub link_density: f32,
    /// TOC-like score
    pub toc_like: f32,
    /// Code block text length
    pub code_block_text_len: usize,
    /// Whether this is a semantic main container
    pub semantic_main_flag: bool,
    /// Positive keyword hits
    pub pos_kw_hits: usize,
    /// Negative keyword hits
    pub neg_kw_hits: usize,
    /// Paragraph count
    pub p_count: usize,
    /// Heading count
    pub h_count: usize,
    /// List item count
    pub li_count: usize,
    /// Depth in tree
    pub depth: u32,
}

/// Build debug information from candidates
pub fn build_debug_info(candidates: &[Candidate], arena: &Arena) -> DebugInfo {
    // Sort by score descending
    let mut sorted: Vec<&Candidate> = candidates.iter().collect();
    sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let all: Vec<CandidateDebug> =
        sorted.into_iter().map(|c| build_candidate_debug(c, arena)).collect();

    DebugInfo { candidates: all }
}

/// Build debug info for a single candidate
fn build_candidate_debug(candidate: &Candidate, arena: &Arena) -> CandidateDebug {
    let node_path = build_node_path(arena, candidate.node_id);
    let features = build_feature_summary(&candidate.features, arena, candidate.node_id);
    let feature_vector = candidate.features.values.to_vec();
    let text = arena.collect_text(candidate.node_id);

    CandidateDebug {
        node_path,
        logit: candidate.score,
        features,
        feature_vector,
        text,
    }
}

/// Build a path string like "html > body > div#main.content > article"
fn build_node_path(arena: &Arena, node_id: NodeId) -> String {
    let mut parts = Vec::new();

    // Collect ancestors (in reverse order)
    let mut current = Some(node_id);
    while let Some(id) = current {
        if let Some(node) = arena.get(id) {
            if let NodeKind::Element { tag, .. } = &node.kind {
                let mut part = tag.as_str().to_string();

                // Add id/class snippets
                if let Some(attrs) = arena.get_attributes(id) {
                    if let Some(ref id_attr) = attrs.id {
                        part.push('#');
                        // Truncate long ids
                        if id_attr.len() > 20 {
                            part.push_str(&id_attr[..20]);
                            part.push_str("...");
                        } else {
                            part.push_str(id_attr);
                        }
                    }

                    if let Some(ref class) = attrs.class {
                        // Take first class only
                        if let Some(first_class) = class.split_whitespace().next() {
                            part.push('.');
                            if first_class.len() > 20 {
                                part.push_str(&first_class[..20]);
                                part.push_str("...");
                            } else {
                                part.push_str(first_class);
                            }
                        }
                    }
                }

                parts.push(part);
            }
            current = node.parent;
        } else {
            break;
        }
    }

    // Reverse and join
    parts.reverse();

    // Limit path length
    if parts.len() > 6 {
        let start: Vec<_> = parts[..2].to_vec();
        let end: Vec<_> = parts[parts.len() - 3..].to_vec();
        format!("{} > ... > {}", start.join(" > "), end.join(" > "))
    } else {
        parts.join(" > ")
    }
}

/// Build feature summary from feature vector
fn build_feature_summary(
    features: &FeatureVector,
    arena: &Arena,
    node_id: NodeId,
) -> FeatureSummary {
    let depth = arena.get(node_id).map(|n| n.depth).unwrap_or(0);

    // Extract values from log1p features
    let text_len = features.get_count(FeatureIndex::LogTextLenChars);
    let code_block_text_len = features.get_count(FeatureIndex::LogCodeBlockTextLen);
    let pos_kw_hits = features.get_count(FeatureIndex::LogPosKwHits);
    let neg_kw_hits = features.get_count(FeatureIndex::LogNegKwHits);
    let p_count = features.get_count(FeatureIndex::LogPCount);
    let h_count = features.get_count(FeatureIndex::LogHCount);
    let li_count = features.get_count(FeatureIndex::LogLiCount);

    FeatureSummary {
        text_len,
        link_density: features.get(FeatureIndex::LinkDensity),
        toc_like: features.get(FeatureIndex::TocLike),
        code_block_text_len,
        semantic_main_flag: features.get(FeatureIndex::SemanticMainFlag) > 0.5,
        pos_kw_hits,
        neg_kw_hits,
        p_count,
        h_count,
        li_count,
        depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::TagId;

    #[test]
    fn test_build_node_path() {
        use crate::dom::{Arena, Node};

        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let html = arena.add_node(Node::element(TagId::Html, None));
        arena.append_child(root, html);

        let body = arena.add_node(Node::element(TagId::Body, None));
        arena.append_child(html, body);

        let div = arena.add_node(Node::element(TagId::Div, None));
        arena.append_child(body, div);
        arena.set_attributes(
            div,
            crate::dom::Attributes {
                id: Some("main".to_string()),
                class: Some("content wrapper".to_string()),
                ..Default::default()
            },
        );

        let article = arena.add_node(Node::element(TagId::Article, None));
        arena.append_child(div, article);

        let path = build_node_path(&arena, article);
        assert!(path.contains("article"));
        assert!(path.contains("div#main.content"));
        assert!(path.contains("body"));
    }
}
