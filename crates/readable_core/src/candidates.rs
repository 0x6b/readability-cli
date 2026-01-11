//! Candidate generation for content extraction
//!
//! Identifies potential content container nodes in the DOM tree.

use crate::{
    dom::{Arena, Attributes, NodeId, NodeKind, TagId},
    features::FeatureVector,
};

/// A candidate node for content extraction
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Node ID in the arena
    pub node_id: NodeId,
    /// Tag of the candidate element
    pub tag: TagId,
    /// Depth in the tree
    pub depth: u32,
    /// Approximate text length (for early filtering)
    pub approx_text_len: usize,
    /// Whether this is a semantic main container
    pub is_semantic: bool,
    /// Feature vector (computed later)
    pub features: FeatureVector,
    /// Logistic regression score (computed later)
    pub score: f32,
}

impl Candidate {
    /// Create a new candidate
    pub fn new(
        node_id: NodeId,
        tag: TagId,
        depth: u32,
        approx_text_len: usize,
        is_semantic: bool,
    ) -> Self {
        Self {
            node_id,
            tag,
            depth,
            approx_text_len,
            is_semantic,
            features: FeatureVector::default(),
            score: f32::NEG_INFINITY,
        }
    }
}

/// Positive keywords for class/id that suggest content
const POSITIVE_KEYWORDS: &[&str] = &[
    "content",
    "main",
    "article",
    "post",
    "entry",
    "body",
    "markdown",
    "readme",
    "doc",
    "docs",
    "documentation",
    "wiki",
    "text",
    "story",
    "hentry",
    "page",
    // Japanese
    "本文",
    "記事",
    "コンテンツ",
];

/// Negative keywords for class/id that suggest boilerplate
const NEGATIVE_KEYWORDS: &[&str] = &[
    "nav",
    "navbar",
    "navigation",
    "sidebar",
    "toc",
    "table-of-contents",
    "breadcrumb",
    "footer",
    "header",
    "comment",
    "comments",
    "share",
    "social",
    "promo",
    "ad",
    "ads",
    "advert",
    "sponsor",
    "related",
    "newsletter",
    "cookie",
    "modal",
    "popup",
    "menu",
    "toolbar",
    "widget",
    "banner",
    "masthead",
    "gallery",
    "slideshow",
    "carousel",
    "lightbox",
    // Japanese
    "ナビ",
    "メニュー",
    "目次",
    "関連",
    "広告",
    "コメント",
];

/// Check if attributes contain positive keywords
fn has_positive_keywords(attrs: Option<&Attributes>) -> bool {
    attrs.map_or(false, |a| a.contains_keyword(POSITIVE_KEYWORDS))
}

/// Check if attributes contain negative keywords
fn has_negative_keywords(attrs: Option<&Attributes>) -> bool {
    attrs.map_or(false, |a| a.contains_keyword(NEGATIVE_KEYWORDS))
}

/// Check if node has role="main"
fn has_role_main(attrs: Option<&Attributes>) -> bool {
    attrs.map_or(false, |a| a.role.as_ref().map_or(false, |r| r.eq_ignore_ascii_case("main")))
}

/// Generate candidates from the arena
pub fn generate_candidates(arena: &Arena, max_candidates: usize) -> Vec<Candidate> {
    let body_id = match arena.find_body() {
        Some(id) => id,
        None => return Vec::new(),
    };

    let mut candidates = Vec::new();
    let mut semantic_candidates = Vec::new();

    // Walk the tree and collect candidates
    collect_candidates_recursive(arena, body_id, &mut candidates, &mut semantic_candidates);

    // Prioritize semantic candidates if we have too many
    if candidates.len() + semantic_candidates.len() > max_candidates {
        // Sort by text length descending, then depth ascending
        candidates.sort_by(|a, b| {
            b.approx_text_len
                .cmp(&a.approx_text_len)
                .then_with(|| a.depth.cmp(&b.depth))
        });

        // Keep all semantic candidates plus fill with best regular candidates
        let remaining = max_candidates.saturating_sub(semantic_candidates.len());
        candidates.truncate(remaining);
    }

    // Combine semantic and regular candidates
    semantic_candidates.extend(candidates);
    semantic_candidates
}

/// Recursively collect candidates from a subtree
fn collect_candidates_recursive(
    arena: &Arena,
    node_id: NodeId,
    candidates: &mut Vec<Candidate>,
    semantic_candidates: &mut Vec<Candidate>,
) {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return,
    };

    if let NodeKind::Element { tag, .. } = &node.kind {
        let attrs = arena.get_attributes(node_id);

        // Skip obvious boilerplate
        if tag.is_boilerplate() {
            return;
        }

        // Skip elements with negative keywords
        if has_negative_keywords(attrs) {
            // Still recurse into children - they might be valid
            for child_id in arena.children(node_id) {
                collect_candidates_recursive(arena, child_id, candidates, semantic_candidates);
            }
            return;
        }

        // Check if this is a candidate tag
        let is_candidate_tag = tag.is_candidate_tag();

        // Check if this is a semantic container
        let is_semantic =
            tag.is_semantic_main() || has_role_main(attrs) || has_positive_keywords(attrs);

        if is_candidate_tag || is_semantic {
            // Compute approximate text length (quick estimate)
            let approx_text_len = estimate_text_length(arena, node_id);

            let candidate = Candidate::new(node_id, *tag, node.depth, approx_text_len, is_semantic);

            if is_semantic {
                semantic_candidates.push(candidate);
            } else {
                candidates.push(candidate);
            }
        }
    }

    // Recurse into children
    for child_id in arena.children(node_id) {
        collect_candidates_recursive(arena, child_id, candidates, semantic_candidates);
    }
}

/// Quick estimate of text length in a subtree (for filtering)
fn estimate_text_length(arena: &Arena, node_id: NodeId) -> usize {
    let mut total = 0;

    for desc_id in arena.descendants(node_id) {
        if let Some(text) = arena.get_text(desc_id) {
            total += text.len();
        }
    }

    // Also count direct text
    if let Some(text) = arena.get_text(node_id) {
        total += text.len();
    }

    total
}

/// Get the positive keywords list (for feature extraction)
pub fn positive_keywords() -> &'static [&'static str] {
    POSITIVE_KEYWORDS
}

/// Get the negative keywords list (for feature extraction)
pub fn negative_keywords() -> &'static [&'static str] {
    NEGATIVE_KEYWORDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_html;

    #[test]
    fn test_generate_candidates_simple() {
        let html = r#"
        <html>
        <body>
            <nav>Navigation</nav>
            <article id="main-content">
                <p>This is the main content of the article.</p>
                <p>More content here.</p>
            </article>
            <aside>Sidebar</aside>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let candidates = generate_candidates(&arena, 100);

        // Should have candidates (article, p elements)
        assert!(!candidates.is_empty());

        // Should have the article as a semantic candidate
        let semantic_count = candidates.iter().filter(|c| c.is_semantic).count();
        assert!(semantic_count > 0);
    }

    #[test]
    fn test_negative_keywords_filter() {
        let html = r#"
        <html>
        <body>
            <div class="sidebar">Should be skipped</div>
            <div class="content">
                <p>Main content</p>
            </div>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let candidates = generate_candidates(&arena, 100);

        // The sidebar div should not be in candidates
        for candidate in &candidates {
            if let Some(attrs) = arena.get_attributes(candidate.node_id) {
                assert!(
                    !attrs.class.as_ref().map_or(false, |c| c.contains("sidebar")),
                    "Sidebar should not be a candidate"
                );
            }
        }
    }

    #[test]
    fn test_candidate_limit() {
        // Create HTML with many divs
        let mut html = String::from("<html><body>");
        for i in 0..1000 {
            html.push_str(&format!("<div>Content {}</div>", i));
        }
        html.push_str("</body></html>");

        let arena = parse_html(&html);
        let candidates = generate_candidates(&arena, 100);

        // Should be limited to max_candidates
        assert!(candidates.len() <= 100);
    }
}
