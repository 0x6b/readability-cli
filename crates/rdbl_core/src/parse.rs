//! HTML parsing into node arena using html5ever
//!
//! Handles encoding detection and conversion for non-UTF-8 pages.

use encoding_rs::Encoding;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::dom::{Arena, Attributes, Node, NodeId, NodeKind, TagId};

/// Parse an HTML string into a node arena
pub fn parse_html(html: &str) -> Arena {
    parse_html_bytes(html.as_bytes())
}

/// Parse HTML bytes into a node arena, handling encoding detection
pub fn parse_html_bytes(bytes: &[u8]) -> Arena {
    // Convert to UTF-8, detecting encoding if needed
    let html = decode_html(bytes);

    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();

    let mut arena = Arena::new();
    let root = arena.add_node(Node::document());
    convert_node(&dom.document, root, &mut arena);
    arena
}

/// Decode HTML bytes to UTF-8 string, detecting encoding from BOM or meta tags
fn decode_html(bytes: &[u8]) -> String {
    // Check for BOM
    if let Some(encoding) = detect_bom(bytes) {
        let (decoded, _, _) = encoding.decode(bytes);
        return decoded.into_owned();
    }

    // Try to detect charset from meta tags (scan first 1024 bytes)
    let scan_len = bytes.len().min(1024);
    if let Some(encoding) = detect_meta_charset(&bytes[..scan_len]) {
        let (decoded, _, _) = encoding.decode(bytes);
        return decoded.into_owned();
    }

    // Default: try UTF-8, fall back to Windows-1252 (superset of ISO-8859-1)
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            decoded.into_owned()
        }
    }
}

/// Detect encoding from BOM (Byte Order Mark)
fn detect_bom(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Some(encoding_rs::UTF_8)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        Some(encoding_rs::UTF_16BE)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        Some(encoding_rs::UTF_16LE)
    } else {
        None
    }
}

/// Detect encoding from meta charset tag
fn detect_meta_charset(bytes: &[u8]) -> Option<&'static Encoding> {
    // Convert to lowercase ASCII for searching
    let lower: Vec<u8> = bytes.iter().map(|&b| b.to_ascii_lowercase()).collect();
    let text = String::from_utf8_lossy(&lower);

    // Look for <meta charset="...">
    if let Some(pos) = text.find("charset") {
        let rest = &text[pos..];
        // Find the value after = or "
        if let Some(start) = rest.find(['"', '\'']) {
            let rest = &rest[start + 1..];
            if let Some(end) = rest.find(['"', '\'']) {
                let charset = rest[..end].trim();
                return Encoding::for_label(charset.as_bytes());
            }
        }
        // Also try charset=value without quotes
        if let Some(eq_pos) = rest.find('=') {
            let rest = &rest[eq_pos + 1..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '>' || c == ';')
                .unwrap_or(rest.len());
            let charset = rest[..end].trim();
            if !charset.is_empty() {
                return Encoding::for_label(charset.as_bytes());
            }
        }
    }

    None
}

/// Recursively convert html5ever nodes to arena nodes
fn convert_node(handle: &Handle, parent_id: NodeId, arena: &mut Arena) {
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Document => {
                // Document node - just process children
                convert_node(child, parent_id, arena);
                continue;
            }

            NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.to_string();
                let tag = TagId::from_name(&tag_name);

                let node =
                    Node::element(tag, if tag == TagId::Other { Some(tag_name) } else { None });
                let id = arena.add_node(node);
                arena.append_child(parent_id, id);

                // Convert attributes
                let attrs_borrow = attrs.borrow();
                if !attrs_borrow.is_empty() {
                    let mut attributes = Attributes::default();
                    for attr in attrs_borrow.iter() {
                        let attr_name = attr.name.local.to_string();
                        let attr_value = attr.value.to_string();

                        match attr_name.as_str() {
                            "id" => attributes.id = Some(attr_value),
                            "class" => attributes.class = Some(attr_value),
                            "role" => attributes.role = Some(attr_value),
                            "href" => attributes.href = Some(attr_value),
                            "src" => attributes.src = Some(attr_value),
                            "alt" => attributes.alt = Some(attr_value),
                            "name" => attributes.name = Some(attr_value),
                            "content" => attributes.content = Some(attr_value),
                            "property" => attributes.property = Some(attr_value),
                            "hidden" => attributes.hidden = true,
                            "style" => attributes.style = Some(attr_value),
                            "aria-hidden" => attributes.aria_hidden = Some(attr_value),
                            _ => {}
                        }
                    }
                    arena.set_attributes(id, attributes);
                }

                // Recursively process children
                convert_node(child, id, arena);
                continue;
            }

            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                // Skip empty text nodes
                if text.trim().is_empty() && !has_preformatted_ancestor(arena, parent_id) {
                    continue;
                }

                let node = Node::text();
                let id = arena.add_node(node);
                arena.append_child(parent_id, id);
                arena.set_text(id, text);
                continue;
            }

            NodeData::Comment { .. } => {
                // Skip comments
                continue;
            }

            NodeData::Doctype { .. } => {
                // Skip doctype
                continue;
            }

            NodeData::ProcessingInstruction { .. } => {
                // Skip processing instructions
                continue;
            }
        };
    }
}

/// Check if a node has a preformatted ancestor (pre, code)
fn has_preformatted_ancestor(arena: &Arena, node_id: NodeId) -> bool {
    for ancestor_id in arena.ancestors(node_id) {
        if let Some(node) = arena.get(ancestor_id)
            && let NodeKind::Element { tag, .. } = &node.kind
            && tag.is_code_container()
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_html() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
<div id="main" class="content">
<p>Hello, world!</p>
</div>
</body>
</html>"#;

        let arena = parse_html(html);

        // Should have nodes
        assert!(!arena.nodes.is_empty());

        // Should have a body
        let body_id = arena.find_body();
        assert!(body_id.is_some());

        // Collect text from body
        let text = arena.collect_text(body_id.unwrap());
        assert!(text.contains("Hello, world!"));
    }

    #[test]
    fn test_parse_with_attributes() {
        let html = r#"<div id="main" class="content article" role="main">Content</div>"#;
        let arena = parse_html(html);

        // Find the div
        for (id, node) in arena.nodes.iter().enumerate() {
            if let NodeKind::Element { tag: TagId::Div, .. } = &node.kind {
                let attrs = arena.get_attributes(id as NodeId);
                assert!(attrs.is_some());
                let attrs = attrs.unwrap();
                assert_eq!(attrs.id.as_deref(), Some("main"));
                assert_eq!(attrs.class.as_deref(), Some("content article"));
                assert_eq!(attrs.role.as_deref(), Some("main"));
                break;
            }
        }
    }

    #[test]
    fn test_parse_preserves_preformatted() {
        let html = r#"<pre><code>  indented
  code</code></pre>"#;
        let arena = parse_html(html);

        // Find the pre element
        let mut pre_id = None;
        for (id, node) in arena.nodes.iter().enumerate() {
            if let NodeKind::Element { tag: TagId::Pre, .. } = &node.kind {
                pre_id = Some(id as NodeId);
                break;
            }
        }

        assert!(pre_id.is_some());
        let text = arena.collect_text(pre_id.unwrap());
        // Should preserve the indentation
        assert!(text.contains("  indented"));
    }

    #[test]
    fn test_detect_meta_charset() {
        // UTF-8
        let html = b"<meta charset=\"utf-8\">";
        assert_eq!(detect_meta_charset(html), Some(encoding_rs::UTF_8));

        // Shift-JIS
        let html = b"<meta charset=\"shift_jis\">";
        assert_eq!(detect_meta_charset(html), Some(encoding_rs::SHIFT_JIS));

        // Content-Type style
        let html = b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=euc-jp\">";
        assert_eq!(detect_meta_charset(html), Some(encoding_rs::EUC_JP));

        // No charset
        let html = b"<html><head><title>Test</title></head></html>";
        assert_eq!(detect_meta_charset(html), None);
    }

    #[test]
    fn test_parse_shift_jis() {
        // Shift-JIS encoded "こんにちは" (Hello in Japanese)
        let html: &[u8] = &[
            0x3C, 0x6D, 0x65, 0x74, 0x61, 0x20, 0x63, 0x68, 0x61, 0x72, 0x73, 0x65, 0x74, 0x3D,
            0x22, 0x73, 0x68, 0x69, 0x66, 0x74, 0x5F, 0x6A, 0x69, 0x73, 0x22,
            0x3E, // <meta charset="shift_jis">
            0x3C, 0x70, 0x3E, // <p>
            0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82,
            0xCD, // こんにちは in Shift-JIS
            0x3C, 0x2F, 0x70, 0x3E, // </p>
        ];

        let arena = parse_html_bytes(html);
        let body_id = arena.find_body().unwrap();
        let text = arena.collect_text(body_id);
        assert!(text.contains("こんにちは"));
    }

    #[test]
    fn test_parse_utf8_bom() {
        // UTF-8 with BOM
        let html: &[u8] = &[
            0xEF, 0xBB, 0xBF, // UTF-8 BOM
            b'<', b'p', b'>', b'H', b'e', b'l', b'l', b'o', b'<', b'/', b'p', b'>',
        ];

        let arena = parse_html_bytes(html);
        let body_id = arena.find_body().unwrap();
        let text = arena.collect_text(body_id);
        assert!(text.contains("Hello"));
    }
}
