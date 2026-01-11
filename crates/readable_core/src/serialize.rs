//! Serialization to HTML and plain text
//!
//! Converts cleaned node trees to sanitized HTML or plain text output.

use crate::{
    cleanup::{CleanedAttrs, CleanedNode, CleanedNodeKind},
    dom::TagId,
};

/// Convert a cleaned tree to sanitized HTML
pub fn to_html(node: &CleanedNode) -> String {
    let mut output = String::new();
    serialize_html_node(node, &mut output, false);
    normalize_html(&output)
}

/// Convert a cleaned tree to plain text
pub fn to_text(node: &CleanedNode) -> String {
    let mut output = String::new();
    serialize_text_node(node, &mut output, false);
    normalize_text(&output)
}

/// Serialize a node to HTML
fn serialize_html_node(node: &CleanedNode, output: &mut String, in_pre: bool) {
    match &node.kind {
        CleanedNodeKind::Text(text) => {
            if in_pre {
                // In preformatted, escape but preserve whitespace
                output.push_str(&escape_html(text));
            } else {
                // Outside pre, normalize whitespace
                let normalized = normalize_whitespace(text);
                if !normalized.is_empty() {
                    output.push_str(&escape_html(&normalized));
                }
            }
        }

        CleanedNodeKind::Element { tag, attrs } => {
            let tag_str = tag.as_str();
            let new_in_pre = in_pre || tag.is_code_container();

            // Skip empty non-void elements
            if node.children.is_empty() && !is_void_element(*tag) {
                return;
            }

            // Opening tag
            output.push('<');
            output.push_str(tag_str);
            serialize_attrs(attrs, *tag, output);
            output.push('>');

            // Void elements don't have closing tags
            if is_void_element(*tag) {
                return;
            }

            // Children
            for child in &node.children {
                serialize_html_node(child, output, new_in_pre);
            }

            // Closing tag
            output.push_str("</");
            output.push_str(tag_str);
            output.push('>');
        }
    }
}

/// Serialize attributes
fn serialize_attrs(attrs: &CleanedAttrs, tag: TagId, output: &mut String) {
    match tag {
        TagId::A => {
            if let Some(href) = &attrs.href {
                output.push_str(" href=\"");
                output.push_str(&escape_attr(href));
                output.push('"');
            }
        }
        TagId::Img => {
            if let Some(src) = &attrs.src {
                output.push_str(" src=\"");
                output.push_str(&escape_attr(src));
                output.push('"');
            }
            if let Some(alt) = &attrs.alt {
                output.push_str(" alt=\"");
                output.push_str(&escape_attr(alt));
                output.push('"');
            }
        }
        _ => {}
    }
}

/// Serialize a node to plain text
fn serialize_text_node(node: &CleanedNode, output: &mut String, in_pre: bool) {
    match &node.kind {
        CleanedNodeKind::Text(text) => {
            if in_pre {
                output.push_str(text);
            } else {
                let normalized = normalize_whitespace(text);
                if !normalized.is_empty() {
                    if !output.is_empty()
                        && !output.ends_with(' ')
                        && !output.ends_with('\n')
                        && !normalized.starts_with(' ')
                    {
                        output.push(' ');
                    }
                    output.push_str(&normalized);
                }
            }
        }

        CleanedNodeKind::Element { tag, .. } => {
            let new_in_pre = in_pre || tag.is_code_container();

            // Add block separator before block elements
            if tag.is_block() && !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }

            // Handle special elements
            match tag {
                TagId::Br => {
                    output.push('\n');
                }
                TagId::Li => {
                    // Add bullet for list items
                    if has_text_content(node) {
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("- ");
                    }
                }
                TagId::Pre | TagId::Code if !in_pre => {
                    // Add code fence for code blocks
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                _ => {}
            }

            // Children
            for child in &node.children {
                serialize_text_node(child, output, new_in_pre);
            }

            // Add block separator after block elements
            if tag.is_block() && !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }

            // Extra newline after paragraphs and headings
            if matches!(
                tag,
                TagId::P
                    | TagId::H1
                    | TagId::H2
                    | TagId::H3
                    | TagId::H4
                    | TagId::H5
                    | TagId::H6
                    | TagId::Blockquote
            ) {
                if !output.ends_with("\n\n") && !output.ends_with('\n') {
                    output.push('\n');
                }
            }
        }
    }
}

fn has_text_content(node: &CleanedNode) -> bool {
    match &node.kind {
        CleanedNodeKind::Text(text) => !text.trim().is_empty(),
        CleanedNodeKind::Element { .. } => node.children.iter().any(has_text_content),
    }
}

/// Escape HTML special characters
fn escape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape attribute values
fn escape_attr(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            _ => result.push(c),
        }
    }
    result
}

/// Normalize whitespace in text (collapse multiple whitespace to single space)
fn normalize_whitespace(text: &str) -> String {
    let mut result = String::new();
    let mut prev_ws = false;
    let mut first_char = true;

    for c in text.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                // Preserve leading whitespace as a single space
                if first_char || !result.is_empty() {
                    result.push(' ');
                }
            }
            prev_ws = true;
        } else {
            result.push(c);
            prev_ws = false;
        }
        first_char = false;
    }

    result
}

/// Check if a tag is a void element (no closing tag)
fn is_void_element(tag: TagId) -> bool {
    matches!(tag, TagId::Br | TagId::Img | TagId::Input | TagId::Meta | TagId::Link)
}

/// Normalize HTML output (remove excessive whitespace between tags)
fn normalize_html(html: &str) -> String {
    // Remove leading/trailing whitespace
    let mut result = html.trim().to_string();

    // Remove multiple consecutive newlines
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result
}

/// Normalize plain text output
fn normalize_text(text: &str) -> String {
    let mut result = text.trim().to_string();

    // Remove more than two consecutive newlines
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    // Remove trailing spaces on lines
    let lines: Vec<&str> = result.lines().map(|l| l.trim_end()).collect();
    result = lines.join("\n");

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_node(text: &str) -> CleanedNode {
        CleanedNode {
            kind: CleanedNodeKind::Text(text.to_string()),
            children: Vec::new(),
        }
    }

    fn make_element_node(tag: TagId, children: Vec<CleanedNode>) -> CleanedNode {
        CleanedNode {
            kind: CleanedNodeKind::Element { tag, attrs: CleanedAttrs::default() },
            children,
        }
    }

    #[test]
    fn test_to_html_simple() {
        let node = make_element_node(TagId::P, vec![make_text_node("Hello, world!")]);

        let html = to_html(&node);
        assert_eq!(html, "<p>Hello, world!</p>");
    }

    #[test]
    fn test_to_html_escapes() {
        let node =
            make_element_node(TagId::P, vec![make_text_node("<script>alert('xss')</script>")]);

        let html = to_html(&node);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_to_html_nested() {
        let inner = make_element_node(TagId::Strong, vec![make_text_node("bold")]);
        let node = make_element_node(
            TagId::P,
            vec![make_text_node("This is "), inner, make_text_node(" text.")],
        );

        let html = to_html(&node);
        assert_eq!(html, "<p>This is <strong>bold</strong> text.</p>");
    }

    #[test]
    fn test_to_html_with_link() {
        let mut link = make_element_node(TagId::A, vec![make_text_node("click here")]);
        if let CleanedNodeKind::Element { ref mut attrs, .. } = link.kind {
            attrs.href = Some("https://example.com".to_string());
        }
        let node = make_element_node(TagId::P, vec![link]);

        let html = to_html(&node);
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("click here"));
    }

    #[test]
    fn test_to_text_simple() {
        let node = make_element_node(TagId::P, vec![make_text_node("Hello, world!")]);

        let text = to_text(&node);
        assert_eq!(text, "Hello, world!");
    }

    #[test]
    fn test_to_text_strips_tags() {
        let inner = make_element_node(TagId::Strong, vec![make_text_node("bold")]);
        let node = make_element_node(
            TagId::P,
            vec![make_text_node("This is "), inner, make_text_node(" text.")],
        );

        let text = to_text(&node);
        assert_eq!(text, "This is bold text.");
    }

    #[test]
    fn test_to_text_list_items() {
        let li1 = make_element_node(TagId::Li, vec![make_text_node("Item 1")]);
        let li2 = make_element_node(TagId::Li, vec![make_text_node("Item 2")]);
        let node = make_element_node(TagId::Ul, vec![li1, li2]);

        let text = to_text(&node);
        assert!(text.contains("- Item 1"));
        assert!(text.contains("- Item 2"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<div>"), "&lt;div&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), " hello world ");
        assert_eq!(normalize_whitespace("\n\t  foo  \n\t  bar  "), " foo bar ");
        assert_eq!(normalize_whitespace("no leading"), "no leading");
    }
}
