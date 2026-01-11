//! HTML parsing into node arena using html5ever

use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::dom::{Arena, Attributes, Node, NodeId, NodeKind, TagId};

/// Parse an HTML string into a node arena
pub fn parse_html(html: &str) -> Arena {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();

    let mut arena = Arena::new();
    let root = arena.add_node(Node::document());
    convert_node(&dom.document, root, &mut arena);
    arena
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
        if let Some(node) = arena.get(ancestor_id) {
            if let NodeKind::Element { tag, .. } = &node.kind {
                if tag.is_code_container() {
                    return true;
                }
            }
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
}
