//! Readable Core - Pure Rust readability extractor using logistic regression
//!
//! This library extracts the main readable content from HTML documents
//! using a logistic regression model over engineered features.

pub mod candidates;
pub mod cleanup;
pub mod debug;
pub mod dom;
pub mod features;
pub mod model;
pub mod parse;
pub mod postprocess;
pub mod select;
pub mod serialize;
pub mod title;

use serde::{Deserialize, Serialize};

/// Options for content extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOptions {
    /// Minimum text characters for a valid candidate (default: 200)
    #[serde(default = "default_min_text_chars")]
    pub min_text_chars: usize,

    /// Maximum candidates to consider (default: 800)
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,

    /// Enable debug output (default: false)
    #[serde(default)]
    pub debug: bool,

    /// Keep table elements in output (default: true)
    #[serde(default = "default_true")]
    pub keep_tables: bool,

    /// Prefer semantic containers (article/main) in tie-breaks (default: true)
    #[serde(default = "default_true")]
    pub prefer_semantic: bool,

    /// Tune thresholds for code-heavy pages (default: true)
    #[serde(default = "default_true")]
    pub code_friendly: bool,
}

fn default_min_text_chars() -> usize {
    200
}
fn default_max_candidates() -> usize {
    800
}
fn default_true() -> bool {
    true
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            min_text_chars: default_min_text_chars(),
            max_candidates: default_max_candidates(),
            debug: false,
            keep_tables: true,
            prefer_semantic: true,
            code_friendly: true,
        }
    }
}

/// Result of content extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResult {
    /// Extracted title
    pub title: Option<String>,

    /// Extracted byline/author
    pub byline: Option<String>,

    /// Sanitized HTML content
    pub content_html: String,

    /// Plain text content
    pub text: String,

    /// Debug information (only present if debug option was enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<debug::DebugInfo>,
}

/// Extract readable content from HTML
///
/// # Arguments
/// * `html` - The HTML string to extract content from
/// * `options` - Extraction options
///
/// # Returns
/// Extracted content including title, HTML, and plain text
pub fn extract(html: &str, options: &ExtractOptions) -> ExtractResult {
    let html_has_body = {
        let lower = html.to_ascii_lowercase();
        lower.contains("<body")
    };

    // Parse HTML into node arena
    let arena = parse::parse_html(html);

    // Extract title early (before cleanup)
    let title = title::extract_title(&arena);
    let byline = title::extract_byline(&arena);

    // Generate candidates
    let mut candidates = candidates::generate_candidates(&arena, options.max_candidates);

    // Compute features for each candidate
    for candidate in &mut candidates {
        candidate.features = features::extract_features(&arena, candidate.node_id);
    }

    // Score candidates with logistic regression
    for candidate in &mut candidates {
        candidate.score = model::score(&candidate.features);
    }

    // Apply score propagation (Readability.js-style parent score accumulation)
    postprocess::apply_score_propagation(&mut candidates, &arena);

    // Select best candidate
    let selected = select::select_best(&candidates, &arena, options);

    // Build debug info if requested
    let debug_info =
        if options.debug { Some(debug::build_debug_info(&candidates, &arena)) } else { None };

    // Get the selected subtree root and find sibling expansions
    let (selected_id, selected_text_len, selected_tag) = if let Some(selected_candidate) = selected {
        let selected_id = selected_candidate.node_id;
        let selected_text_len = (selected_candidate.features.values
            [features::FeatureIndex::LogTextLenChars as usize]
            .exp()
            - 1.0) as usize;

        let mut selected_id = Some(selected_id);
        let mut selected_text_len = selected_text_len;
        let mut selected_tag = Some(selected_candidate.tag);

        if html_has_body && matches!(selected_tag, Some(dom::TagId::P | dom::TagId::Td)) {
            if let Some(body_id) = arena.find_body() {
                let body_features = features::extract_features(&arena, body_id);
                let body_text_len =
                    (body_features.values[features::FeatureIndex::LogTextLenChars as usize].exp()
                        - 1.0) as usize;
                let body_link_density =
                    body_features.values[features::FeatureIndex::LinkDensity as usize];
                let body_toc_like =
                    body_features.values[features::FeatureIndex::TocLike as usize];

                if body_text_len > selected_text_len * 3
                    && body_link_density < 0.05
                    && body_toc_like < 0.2
                {
                    selected_id = Some(body_id);
                    selected_text_len = body_text_len;
                    selected_tag = Some(dom::TagId::Body);
                }
            }
        }

        (selected_id, selected_text_len, selected_tag)
    } else if html_has_body {
        if let Some(body_id) = arena.find_body() {
            let body_features = features::extract_features(&arena, body_id);
            let body_text_len =
                (body_features.values[features::FeatureIndex::LogTextLenChars as usize].exp()
                    - 1.0) as usize;
            let body_link_density =
                body_features.values[features::FeatureIndex::LinkDensity as usize];
            let body_toc_like = body_features.values[features::FeatureIndex::TocLike as usize];

            if body_text_len > 0 && body_link_density < 0.05 && body_toc_like < 0.2 {
                (Some(body_id), body_text_len, Some(dom::TagId::Body))
            } else {
                (None, 0, None)
            }
        } else {
            (None, 0, None)
        }
    } else {
        (None, 0, None)
    };

    let (content_roots, use_sibling_expansion) = if let Some(selected_id) = selected_id {
        // Find siblings that should be included
        let siblings = if matches!(selected_tag, Some(dom::TagId::Article)) {
            Vec::new()
        } else if matches!(selected_tag, Some(dom::TagId::Section)) {
            let mut section_siblings = 0usize;
            if let Some(parent_id) = arena.get(selected_id).and_then(|n| n.parent) {
                for child_id in arena.children(parent_id) {
                    if let Some(node) = arena.get(child_id) {
                        if matches!(
                            node.kind,
                            dom::NodeKind::Element { tag: dom::TagId::Section, .. }
                        ) {
                            section_siblings += 1;
                        }
                    }
                }
            }

            if section_siblings > 1 {
                Vec::new()
            } else {
                postprocess::find_expansion_siblings(&arena, selected_id, selected_text_len)
            }
        } else {
            postprocess::find_expansion_siblings(&arena, selected_id, selected_text_len)
        };

        if siblings.is_empty() {
            (vec![selected_id], false)
        } else {
            // Include selected node plus siblings in document order
            let mut all_roots = Vec::new();

            // Find position of selected in parent's children
            if let Some(parent_id) = arena.get(selected_id).and_then(|n| n.parent) {
                for child_id in arena.children(parent_id) {
                    if child_id == selected_id || siblings.contains(&child_id) {
                        all_roots.push(child_id);
                    }
                }
            }

            if all_roots.is_empty() {
                all_roots.push(selected_id);
            }

            (all_roots, true)
        }
    } else {
        (Vec::new(), false)
    };

    // Cleanup and serialize
    let (content_html, text) = if content_roots.is_empty() {
        (String::new(), String::new())
    } else if content_roots.len() == 1 && !use_sibling_expansion {
        // Single root - standard path
        let root_id = content_roots[0];
        let cleaned = cleanup::cleanup(&arena, root_id, options);
        let html = serialize::to_html(&cleaned);
        let text = serialize::to_text(&cleaned);
        (html, text)
    } else {
        // Multiple roots from sibling expansion - merge them
        let mut combined_html = String::new();
        let mut combined_text = String::new();

        for root_id in &content_roots {
            let cleaned = cleanup::cleanup(&arena, *root_id, options);
            combined_html.push_str(&serialize::to_html(&cleaned));
            combined_html.push('\n');
            combined_text.push_str(&serialize::to_text(&cleaned));
            combined_text.push('\n');
        }

        (combined_html, combined_text)
    };

    ExtractResult {
        title,
        byline,
        content_html,
        text,
        debug: debug_info,
    }
}
