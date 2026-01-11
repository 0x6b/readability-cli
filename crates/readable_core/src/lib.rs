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

    // Select best candidate
    let selected = select::select_best(&candidates, &arena, options);

    // Build debug info if requested
    let debug_info =
        if options.debug { Some(debug::build_debug_info(&candidates, &arena)) } else { None };

    // Get the selected subtree root
    let content_root = selected.map(|c| c.node_id);

    // Cleanup and serialize
    let (content_html, text) = if let Some(root_id) = content_root {
        let cleaned = cleanup::cleanup(&arena, root_id, options);
        let html = serialize::to_html(&cleaned);
        let text = serialize::to_text(&cleaned);
        (html, text)
    } else {
        (String::new(), String::new())
    };

    ExtractResult {
        title,
        byline,
        content_html,
        text,
        debug: debug_info,
    }
}
