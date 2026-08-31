//! Title and byline extraction
//!
//! Extracts title and author information from HTML documents.

use crate::dom::{Arena, NodeId, TagId};
use serde_json::Value;

/// Extract the title from the document
pub fn extract_title(arena: &Arena) -> Option<String> {
    let body_heading = arena
        .find_body()
        .and_then(|body_id| find_first_heading(arena, body_id, TagId::H1))
        .map(|title| clean_title(&title));

    if let Some(metadata) = find_json_ld_metadata(arena)
        && let Some(headline) = metadata.headline
    {
        return Some(clean_title(&headline));
    }

    if let Some(head_id) = arena.find_head() {
        let head_titles = [
            find_meta_content(arena, head_id, "og:title", true),
            find_meta_content(arena, head_id, "twitter:title", false),
            find_title_tag(arena, head_id),
        ];
        let first = head_titles.iter().flatten().next().map(|title| clean_title(title));

        if let (Some(head_title), Some(heading)) = (&first, &body_heading)
            && heading.chars().count() >= head_title.chars().count() + 8
            && heading.chars().count() <= 200
            && head_titles
                .iter()
                .flatten()
                .all(|title| clean_title(title).eq_ignore_ascii_case(head_title))
        {
            return Some(heading.clone());
        }

        if first.is_some() {
            return first;
        }
    }

    body_heading
}

/// Extract byline/author from the document
pub fn extract_byline(arena: &Arena) -> Option<String> {
    if let Some(author) = find_json_ld_metadata(arena).and_then(|metadata| metadata.author) {
        return Some(author);
    }

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

#[derive(Default)]
struct StructuredMetadata {
    headline: Option<String>,
    author: Option<String>,
}

fn find_json_ld_metadata(arena: &Arena) -> Option<StructuredMetadata> {
    for node_id in arena.descendants(0) {
        if arena.get(node_id).and_then(|node| node.tag()) != Some(TagId::Script)
            || !arena.get_attributes(node_id).is_some_and(|attrs| {
                attrs
                    .type_attr
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case("application/ld+json"))
            })
        {
            continue;
        }

        let text = arena.collect_text(node_id);
        if let Ok(value) = serde_json::from_str::<Value>(text.trim())
            && let Some(metadata) = metadata_from_json(&value)
        {
            return Some(metadata);
        }
    }
    None
}

fn metadata_from_json(value: &Value) -> Option<StructuredMetadata> {
    match value {
        Value::Array(values) => values.iter().find_map(metadata_from_json),
        Value::Object(object) => {
            if let Some(headline) = object.get("headline").and_then(Value::as_str) {
                return Some(StructuredMetadata {
                    headline: Some(headline.to_string()),
                    author: object.get("author").and_then(author_from_json),
                });
            }
            object.values().find_map(metadata_from_json)
        }
        _ => None,
    }
}

fn author_from_json(value: &Value) -> Option<String> {
    match value {
        Value::String(author) => nonempty(author),
        Value::Object(object) => object.get("name").and_then(Value::as_str).and_then(nonempty),
        Value::Array(authors) => {
            let names: Vec<_> = authors.iter().filter_map(author_from_json).collect();
            (!names.is_empty()).then(|| names.join(", "))
        }
        _ => None,
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Find meta content by property attribute
fn find_meta_content(
    arena: &Arena,
    head_id: NodeId,
    property: &str,
    use_property: bool,
) -> Option<String> {
    for (child_id, tag) in arena.child_elements(head_id) {
        if tag == TagId::Meta
            && let Some(attrs) = arena.get_attributes(child_id)
        {
            let matches = if use_property {
                attrs
                    .property
                    .as_ref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(property))
            } else {
                attrs.name.as_ref().is_some_and(|n| n.eq_ignore_ascii_case(property))
            };

            if matches
                && let Some(content) = &attrs.content
                && !content.trim().is_empty()
            {
                return Some(content.clone());
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
    fn test_extract_title_prefers_substantive_h1_over_generic_head_metadata() {
        let html = r#"
        <html><head>
            <meta property="og:title" content="Jane Doe">
            <meta name="twitter:title" content="Jane Doe">
            <title>Jane Doe</title>
        </head><body>
            <h1>Training a Small Model to Paint with Code</h1>
        </body></html>
        "#;

        let arena = parse_html(html);
        assert_eq!(
            extract_title(&arena),
            Some("Training a Small Model to Paint with Code".to_string())
        );
    }

    #[test]
    fn test_extracts_json_ld_metadata() {
        let html = r#"
        <html><head>
            <script type="application/ld+json">
            {
                "@type": "Article",
                "headline": "Structured Article Title",
                "author": [{"@type": "Person", "name": "Jane Doe"}, "John Roe"]
            }
            </script>
            <meta property="og:title" content="Fallback title">
        </head><body><h1>Visible title</h1></body></html>
        "#;

        let arena = parse_html(html);
        assert_eq!(extract_title(&arena), Some("Structured Article Title".to_string()));
        assert_eq!(extract_byline(&arena), Some("Jane Doe, John Roe".to_string()));
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
