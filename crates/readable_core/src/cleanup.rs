//! Cleanup pass for removing boilerplate from selected content
//!
//! Applies deterministic rules to remove navigation, ads, and other noise.

use std::collections::HashSet;

use crate::{
    ExtractOptions,
    blocklist::should_block,
    dom::{Arena, NodeId, NodeKind, TagId},
    keywords::{negative_keywords, positive_keywords},
    postprocess::{compute_link_text_len, is_boilerplate_for_cleanup},
};

/// A cleaned node for output
#[derive(Debug, Clone)]
pub struct CleanedNode {
    pub kind: CleanedNodeKind,
    pub children: Vec<CleanedNode>,
}

/// Kind of cleaned node
#[derive(Debug, Clone)]
pub enum CleanedNodeKind {
    /// Element with tag and attributes
    Element { tag: TagId, attrs: CleanedAttrs },
    /// Text content
    Text(String),
}

/// Cleaned attributes (only whitelisted)
#[derive(Debug, Clone, Default)]
pub struct CleanedAttrs {
    pub href: Option<String>,
}

/// Tags allowed in output
const ALLOWED_TAGS: &[TagId] = &[
    TagId::P,
    TagId::Br,
    TagId::H1,
    TagId::H2,
    TagId::H3,
    TagId::H4,
    TagId::H5,
    TagId::H6,
    TagId::Ul,
    TagId::Ol,
    TagId::Li,
    TagId::Pre,
    TagId::Code,
    TagId::Blockquote,
    TagId::Em,
    TagId::Strong,
    TagId::A,
    TagId::Figure,
    TagId::Figcaption,
    TagId::Kbd,
    TagId::Samp,
    TagId::Var,
    TagId::Mark,
    TagId::Small,
    TagId::Sub,
    TagId::Sup,
    TagId::B,
    TagId::I,
    TagId::U,
];

/// Table tags (optional)
const TABLE_TAGS: &[TagId] =
    &[TagId::Table, TagId::Thead, TagId::Tbody, TagId::Tr, TagId::Th, TagId::Td];

/// Tags to completely remove (with contents)
const REMOVE_TAGS: &[TagId] = &[
    TagId::Script,
    TagId::Style,
    TagId::Noscript,
    TagId::Iframe,
    TagId::Form,
    TagId::Button,
    TagId::Input,
    TagId::Select,
    TagId::Textarea,
    TagId::Img,
];

/// Tags to remove but keep children
const UNWRAP_TAGS: &[TagId] =
    &[TagId::Div, TagId::Section, TagId::Article, TagId::Main, TagId::Span];

/// Cleanup the selected subtree
pub fn cleanup(arena: &Arena, root_id: NodeId, options: &ExtractOptions) -> CleanedNode {
    // Build set of nodes to remove
    let mut remove_set = HashSet::new();
    mark_for_removal(arena, root_id, &mut remove_set, options);

    // Build allowed tag set
    let mut allowed: HashSet<TagId> = ALLOWED_TAGS.iter().copied().collect();
    if options.keep_tables {
        for tag in TABLE_TAGS {
            allowed.insert(*tag);
        }
    }

    let unwrap: HashSet<TagId> = UNWRAP_TAGS.iter().copied().collect();

    // Build cleaned tree
    build_cleaned_node(arena, root_id, &remove_set, &allowed, &unwrap)
}

/// Mark nodes for removal
fn mark_for_removal(
    arena: &Arena,
    node_id: NodeId,
    remove_set: &mut HashSet<NodeId>,
    _options: &ExtractOptions,
) {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return,
    };

    if let Some(tag) = node.tag() {
        // Remove certain tags completely
        if REMOVE_TAGS.contains(&tag) {
            remove_set.insert(node_id);
            return;
        }

        // Enhanced boilerplate detection using postprocess module
        if is_boilerplate_for_cleanup(arena, node_id) {
            remove_set.insert(node_id);
            return;
        }

        // Curated blocklist for common boilerplate patterns
        if should_block(arena, node_id) {
            remove_set.insert(node_id);
            return;
        }

        // Check for negative keywords in class/id
        if let Some(attrs) = arena.get_attributes(node_id) {
            if attrs.contains_keyword(&["mwe-math", "mathml"]) {
                remove_set.insert(node_id);
                return;
            }

            if tag == TagId::Section && attrs.contains_keyword(&["5-band"]) {
                let is_opinion = attrs.contains_keyword(&["5-band-intl-opinion"]);
                prune_band_section_text(arena, node_id, remove_set, is_opinion);
            }

            if attrs.contains_keyword(&["highlight", "highlights"])
                && let Some(primary_child) = find_primary_highlight_child(arena, node_id)
            {
                for (child_id, _) in arena.child_elements(node_id) {
                    if child_id != primary_child {
                        remove_set.insert(child_id);
                    }
                }
            }

            if attrs.contains_keyword(negative_keywords()) {
                // Don't remove if also has positive keywords
                if !attrs.contains_keyword(positive_keywords()) {
                    // Check if this is a significant container - if so, be more careful
                    let text = arena.collect_text(node_id);
                    if text.chars().count() < 200 {
                        remove_set.insert(node_id);
                        return;
                    }
                }
            }

            // Drop large gallery-style blocks even if they have lots of text
            if attrs.contains_keyword(&["gallery", "slideshow", "carousel", "lightbox"]) {
                let (img_count, li_count) = count_descendant_media(arena, node_id);
                if img_count >= 5 || li_count >= 10 {
                    remove_set.insert(node_id);
                    return;
                }
            }
        }

        // Remove high link density micro-blocks
        if is_link_heavy_microblock(arena, node_id) {
            remove_set.insert(node_id);
            return;
        }
    }

    // Recurse into children
    for child_id in arena.children(node_id) {
        mark_for_removal(arena, child_id, remove_set, _options);
    }
}

fn count_descendant_media(arena: &Arena, node_id: NodeId) -> (usize, usize) {
    let mut img_count = 0;
    let mut li_count = 0;

    for desc_id in arena.descendants(node_id) {
        if let Some(tag) = arena.get(desc_id).and_then(|n| n.tag()) {
            match tag {
                TagId::Img => img_count += 1,
                TagId::Li => li_count += 1,
                _ => {}
            }
        }
    }

    (img_count, li_count)
}

/// Check if a node is a small block with mostly links
fn is_link_heavy_microblock(arena: &Arena, node_id: NodeId) -> bool {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return false,
    };

    let tag = match node.tag() {
        Some(t) => t,
        None => return false,
    };

    // Preserve headings even if they're wrapped in links.
    if tag.is_heading() {
        return false;
    }

    if has_heading_descendant(arena, node_id) {
        return false;
    }

    if tag == TagId::A
        && let Some(parent) = arena.parent(node_id).and_then(|pid| arena.get(pid))
        && let Some(parent_tag) = parent.tag()
    {
        if matches!(parent_tag, TagId::Header) {
            return false;
        }
        if parent_tag.is_heading() {
            return false;
        }
        if matches!(parent_tag, TagId::P | TagId::Blockquote) {
            return false;
        }
    }

    let text = arena.collect_text(node_id);
    let text_len = text.chars().count();

    // Only check small blocks
    if text_len > 150 {
        return false;
    }

    // Count link text
    let link_text_len = compute_link_text_len(arena, node_id);

    // High link density = likely nav
    let link_density = link_text_len as f32 / text_len.max(1) as f32;
    link_density > 0.7 && text_len > 20
}

fn find_primary_highlight_child(arena: &Arena, node_id: NodeId) -> Option<NodeId> {
    arena
        .child_elements(node_id)
        .find(|&(cid, _)| has_heading_descendant(arena, cid))
        .map(|(cid, _)| cid)
}

fn has_heading_descendant(arena: &Arena, node_id: NodeId) -> bool {
    arena.has_descendant_with_tags(node_id, &[TagId::H1, TagId::H2, TagId::H3])
}

fn prune_band_section_text(
    arena: &Arena,
    section_id: NodeId,
    remove_set: &mut HashSet<NodeId>,
    is_opinion: bool,
) {
    let mut text_blocks = Vec::new();
    let mut article_count = 0;

    for desc_id in arena.descendants(section_id) {
        if arena.get(desc_id).and_then(|n| n.tag()) == Some(TagId::Article) {
            article_count += 1;
            if let Some(text_container) = find_article_text_container(arena, desc_id) {
                let headline_len =
                    find_headline_length(arena, text_container).unwrap_or(usize::MAX);
                text_blocks.push((text_container, headline_len));
            }
        }
    }

    if article_count < 3 || text_blocks.is_empty() {
        return;
    }

    if is_opinion {
        for (text_container, _) in text_blocks {
            remove_set.insert(text_container);
        }
        return;
    }

    let mut keep_container = None;
    let mut keep_len = usize::MAX;
    for (text_container, headline_len) in &text_blocks {
        if *headline_len < keep_len {
            keep_len = *headline_len;
            keep_container = Some(*text_container);
        }
    }

    if let Some(keep_container) = keep_container {
        for (text_container, _) in text_blocks {
            if text_container != keep_container {
                remove_set.insert(text_container);
            }
        }
    }
}

fn find_article_text_container(arena: &Arena, article_id: NodeId) -> Option<NodeId> {
    arena
        .child_elements(article_id)
        .find(|&(cid, tag)| tag == TagId::Div && has_textual_descendant(arena, cid))
        .map(|(cid, _)| cid)
}

fn find_headline_length(arena: &Arena, node_id: NodeId) -> Option<usize> {
    for desc_id in arena.descendants(node_id) {
        if arena.get(desc_id).and_then(|n| n.tag()) == Some(TagId::H2) {
            let text = arena.collect_text(desc_id);
            let len = text.chars().count();
            if len > 0 {
                return Some(len);
            }
        }
    }
    None
}

fn has_textual_descendant(arena: &Arena, node_id: NodeId) -> bool {
    for desc_id in arena.descendants(node_id) {
        if let Some(tag) = arena.get(desc_id).and_then(|n| n.tag())
            && (tag == TagId::P || tag.is_heading())
        {
            return true;
        }
    }
    false
}

/// Build a cleaned node tree
fn build_cleaned_node(
    arena: &Arena,
    node_id: NodeId,
    remove_set: &HashSet<NodeId>,
    allowed: &HashSet<TagId>,
    unwrap: &HashSet<TagId>,
) -> CleanedNode {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => {
            return CleanedNode {
                kind: CleanedNodeKind::Text(String::new()),
                children: Vec::new(),
            };
        }
    };

    match &node.kind {
        NodeKind::Text => {
            let text = arena.get_text(node_id).unwrap_or("").to_string();
            CleanedNode {
                kind: CleanedNodeKind::Text(text),
                children: Vec::new(),
            }
        }

        NodeKind::Element { tag, .. } => {
            // Collect cleaned children
            let mut children = Vec::new();
            for child_id in arena.children(node_id) {
                if remove_set.contains(&child_id) {
                    continue;
                }

                // Check if child element should be removed
                if let Some(child_tag) = arena.get(child_id).and_then(|c| c.tag())
                    && REMOVE_TAGS.contains(&child_tag)
                {
                    continue;
                }

                let cleaned = build_cleaned_node(arena, child_id, remove_set, allowed, unwrap);

                // Flatten if this child is unwrapped
                if let CleanedNodeKind::Element { tag: child_tag, .. } = &cleaned.kind
                    && unwrap.contains(child_tag)
                    && !allowed.contains(child_tag)
                {
                    // Unwrap: add children directly
                    children.extend(cleaned.children);
                    continue;
                }

                children.push(cleaned);
            }

            // Check if this tag is allowed
            if allowed.contains(tag) {
                let attrs = build_cleaned_attrs(arena, node_id, *tag);
                CleanedNode {
                    kind: CleanedNodeKind::Element { tag: *tag, attrs },
                    children,
                }
            } else if unwrap.contains(tag) {
                // Return a wrapper that will be flattened
                CleanedNode {
                    kind: CleanedNodeKind::Element { tag: *tag, attrs: CleanedAttrs::default() },
                    children,
                }
            } else {
                // Keep as div-like container
                CleanedNode {
                    kind: CleanedNodeKind::Element {
                        tag: TagId::Div,
                        attrs: CleanedAttrs::default(),
                    },
                    children,
                }
            }
        }

        NodeKind::Document => {
            let mut children = Vec::new();
            for child_id in arena.children(node_id) {
                if !remove_set.contains(&child_id) {
                    let cleaned = build_cleaned_node(arena, child_id, remove_set, allowed, unwrap);
                    children.push(cleaned);
                }
            }
            CleanedNode {
                kind: CleanedNodeKind::Element { tag: TagId::Div, attrs: CleanedAttrs::default() },
                children,
            }
        }

        NodeKind::Comment => CleanedNode {
            kind: CleanedNodeKind::Text(String::new()),
            children: Vec::new(),
        },
    }
}

/// Build cleaned attributes (only whitelisted)
fn build_cleaned_attrs(arena: &Arena, node_id: NodeId, tag: TagId) -> CleanedAttrs {
    let attrs = match arena.get_attributes(node_id) {
        Some(a) => a,
        None => return CleanedAttrs::default(),
    };

    let mut cleaned = CleanedAttrs::default();

    if tag == TagId::A {
        // Keep href, but filter out javascript: URLs
        if let Some(href) = &attrs.href
            && !href.trim_start().to_lowercase().starts_with("javascript:")
        {
            cleaned.href = Some(href.clone());
        }
    }

    cleaned
}

/// Flatten a cleaned tree to remove empty containers
pub fn flatten_cleaned_tree(node: CleanedNode) -> Option<CleanedNode> {
    match node.kind {
        CleanedNodeKind::Text(ref text) => {
            // Remove empty text nodes
            if text.trim().is_empty() { None } else { Some(node) }
        }
        CleanedNodeKind::Element { tag, attrs } => {
            // Recursively flatten children
            let children: Vec<CleanedNode> =
                node.children.into_iter().filter_map(flatten_cleaned_tree).collect();

            // Remove empty containers (except self-closing tags like Br)
            if children.is_empty() && tag != TagId::Br {
                return None;
            }

            Some(CleanedNode {
                kind: CleanedNodeKind::Element { tag, attrs },
                children,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_html;
    #[cfg(test)]
    use crate::serialize::to_html;

    #[test]
    fn test_cleanup_removes_script() {
        let html = r#"
        <html>
        <body>
            <div id="content">
                <p>Main content</p>
                <script>alert('hello');</script>
                <p>More content</p>
            </div>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let body_id = arena.find_body().unwrap();

        let options = ExtractOptions::default();
        let cleaned = cleanup(&arena, body_id, &options);

        // Serialize to check script is removed
        let html = to_html(&cleaned);
        assert!(!html.contains("script"));
        assert!(!html.contains("alert"));
        assert!(html.contains("Main content"));
    }

    #[test]
    fn test_cleanup_removes_nav() {
        let html = r#"
        <html>
        <body>
            <nav>Navigation links</nav>
            <article>
                <p>Article content</p>
            </article>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let body_id = arena.find_body().unwrap();

        let options = ExtractOptions::default();
        let cleaned = cleanup(&arena, body_id, &options);

        let html = to_html(&cleaned);
        assert!(!html.contains("Navigation"));
        assert!(html.contains("Article content"));
    }

    #[test]
    fn test_cleanup_preserves_code() {
        let html = r#"
        <html>
        <body>
            <article>
                <pre><code>fn main() {
    println!("Hello");
}</code></pre>
            </article>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let body_id = arena.find_body().unwrap();

        let options = ExtractOptions::default();
        let cleaned = cleanup(&arena, body_id, &options);

        let html = to_html(&cleaned);
        assert!(html.contains("<pre>"));
        assert!(html.contains("<code>"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_cleanup_filters_javascript_links() {
        let html = r#"
        <html>
        <body>
            <p>
                <a href="https://example.com">Good link</a>
                <a href="javascript:alert('xss')">Bad link</a>
            </p>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let body_id = arena.find_body().unwrap();

        let options = ExtractOptions::default();
        let cleaned = cleanup(&arena, body_id, &options);

        let html = to_html(&cleaned);
        assert!(html.contains("https://example.com"));
        assert!(!html.contains("javascript:"));
    }
}
