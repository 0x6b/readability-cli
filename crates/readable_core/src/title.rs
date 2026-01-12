//! Title and byline extraction
//!
//! Extracts title and author information from HTML documents.

use crate::dom::{Arena, NodeId, TagId};

/// Extract the title from the document
pub fn extract_title(arena: &Arena) -> Option<String> {
    // Try in order: og:title, twitter:title, <title>, first h1

    // 1. Check meta tags in head
    if let Some(head_id) = arena.find_head() {
        // Look for og:title
        if let Some(title) = find_meta_content(arena, head_id, "og:title", true) {
            return Some(clean_title(&title));
        }

        // Look for twitter:title
        if let Some(title) = find_meta_content(arena, head_id, "twitter:title", false) {
            return Some(clean_title(&title));
        }

        // Look for <title> tag
        if let Some(title) = find_title_tag(arena, head_id) {
            return Some(clean_title(&title));
        }
    }

    // 2. Look for first h1 in body
    if let Some(body_id) = arena.find_body() {
        if let Some(h1_text) = find_first_heading(arena, body_id, TagId::H1) {
            return Some(clean_title(&h1_text));
        }
    }

    None
}

/// Extract byline/author from the document
pub fn extract_byline(arena: &Arena) -> Option<String> {
    // Check meta tags
    if let Some(head_id) = arena.find_head() {
        // Look for author meta tag
        if let Some(author) = find_meta_by_name(arena, head_id, "author") {
            return Some(author);
        }

        // Look for article:author
        if let Some(author) = find_meta_content(arena, head_id, "article:author", true) {
            return Some(author);
        }
    }

    // TODO: Look for common byline patterns in body (future enhancement)

    None
}

/// Find meta content by property attribute
fn find_meta_content(
    arena: &Arena,
    head_id: NodeId,
    property: &str,
    use_property: bool,
) -> Option<String> {
    for (child_id, tag) in arena.child_elements(head_id) {
        if tag == TagId::Meta {
            if let Some(attrs) = arena.get_attributes(child_id) {
                let matches = if use_property {
                    attrs
                        .property
                        .as_ref()
                        .map_or(false, |p| p.eq_ignore_ascii_case(property))
                } else {
                    attrs
                        .name
                        .as_ref()
                        .map_or(false, |n| n.eq_ignore_ascii_case(property))
                };

                if matches {
                    if let Some(content) = &attrs.content {
                        if !content.trim().is_empty() {
                            return Some(content.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Find meta content by name attribute
fn find_meta_by_name(arena: &Arena, head_id: NodeId, name: &str) -> Option<String> {
    find_meta_content(arena, head_id, name, false)
}

/// Find the <title> tag content
fn find_title_tag(arena: &Arena, head_id: NodeId) -> Option<String> {
    for (child_id, tag) in arena.child_elements(head_id) {
        if tag == TagId::Title {
            let text = arena.collect_text(child_id);
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Find the first heading of a specific level
fn find_first_heading(arena: &Arena, node_id: NodeId, target_tag: TagId) -> Option<String> {
    for desc_id in arena.descendants(node_id) {
        if arena.get(desc_id).and_then(|n| n.tag()) == Some(target_tag) {
            let text = arena.collect_text(desc_id);
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Clean up title text
fn clean_title(title: &str) -> String {
    let mut result = title.trim().to_string();

    // Remove common suffixes like " | Site Name" or " - Site Name"
    for separator in &[" | ", " - ", " :: ", " — ", " – "] {
        if let Some(pos) = result.rfind(separator) {
            // Only remove if the suffix looks like a site name (shorter than main title)
            let before = &result[..pos];
            let after = &result[pos + separator.len()..];

            // If what's after is shorter and the before part is substantial, remove it
            if after.len() < before.len() && before.len() > 10 {
                result = before.to_string();
            }
        }
    }

    // Normalize whitespace
    let parts: Vec<&str> = result.split_whitespace().collect();
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_html;

    #[test]
    fn test_extract_title_from_og() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta property="og:title" content="Test Article Title">
            <title>Test Article Title | My Site</title>
        </head>
        <body><p>Content</p></body>
        </html>
        "#;

        let arena = parse_html(html);
        let title = extract_title(&arena);
        assert_eq!(title, Some("Test Article Title".to_string()));
    }

    #[test]
    fn test_extract_title_from_title_tag() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Page Title</title>
        </head>
        <body><p>Content</p></body>
        </html>
        "#;

        let arena = parse_html(html);
        let title = extract_title(&arena);
        assert_eq!(title, Some("Page Title".to_string()));
    }

    #[test]
    fn test_extract_title_cleans_suffix() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Great Article About Rust | Programming Blog</title>
        </head>
        <body><p>Content</p></body>
        </html>
        "#;

        let arena = parse_html(html);
        let title = extract_title(&arena);
        assert_eq!(title, Some("Great Article About Rust".to_string()));
    }

    #[test]
    fn test_extract_byline() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta name="author" content="John Doe">
        </head>
        <body><p>Content</p></body>
        </html>
        "#;

        let arena = parse_html(html);
        let byline = extract_byline(&arena);
        assert_eq!(byline, Some("John Doe".to_string()));
    }

    #[test]
    fn test_clean_title() {
        assert_eq!(clean_title("Article Title | Site Name"), "Article Title");
        assert_eq!(clean_title("Article Title - Blog"), "Article Title");
        assert_eq!(clean_title("  Multiple   Spaces  "), "Multiple Spaces");
    }
}
