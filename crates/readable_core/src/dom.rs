//! DOM-free node arena data structures
//!
//! Uses index-based references instead of pointers for WASM compatibility.

use std::collections::HashMap;

/// Node identifier - index into the arena
pub type NodeId = u32;

/// Tag identifier for common HTML tags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TagId {
    // Document structure
    Html,
    Head,
    Body,

    // Semantic containers
    Article,
    Main,
    Section,
    Aside,
    Nav,
    Header,
    Footer,

    // Content blocks
    Div,
    P,
    Pre,
    Blockquote,

    // Headings
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,

    // Lists
    Ul,
    Ol,
    Li,

    // Tables
    Table,
    Thead,
    Tbody,
    Tr,
    Th,
    Td,

    // Inline elements
    A,
    Span,
    Em,
    Strong,
    B,
    I,
    U,
    Code,
    Kbd,
    Samp,
    Var,
    Mark,
    Small,
    Sub,
    Sup,
    Br,

    // Media
    Img,
    Figure,
    Figcaption,
    Video,
    Audio,

    // Forms
    Form,
    Input,
    Button,
    Select,
    Textarea,
    Label,

    // Other
    Script,
    Style,
    Noscript,
    Iframe,
    Meta,
    Link,
    Title,

    // Catch-all for unknown tags
    Other,
}

impl TagId {
    /// Parse a tag name string into a TagId
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "html" => TagId::Html,
            "head" => TagId::Head,
            "body" => TagId::Body,
            "article" => TagId::Article,
            "main" => TagId::Main,
            "section" => TagId::Section,
            "aside" => TagId::Aside,
            "nav" => TagId::Nav,
            "header" => TagId::Header,
            "footer" => TagId::Footer,
            "div" => TagId::Div,
            "p" => TagId::P,
            "pre" => TagId::Pre,
            "blockquote" => TagId::Blockquote,
            "h1" => TagId::H1,
            "h2" => TagId::H2,
            "h3" => TagId::H3,
            "h4" => TagId::H4,
            "h5" => TagId::H5,
            "h6" => TagId::H6,
            "ul" => TagId::Ul,
            "ol" => TagId::Ol,
            "li" => TagId::Li,
            "table" => TagId::Table,
            "thead" => TagId::Thead,
            "tbody" => TagId::Tbody,
            "tr" => TagId::Tr,
            "th" => TagId::Th,
            "td" => TagId::Td,
            "a" => TagId::A,
            "span" => TagId::Span,
            "em" => TagId::Em,
            "strong" => TagId::Strong,
            "b" => TagId::B,
            "i" => TagId::I,
            "u" => TagId::U,
            "code" => TagId::Code,
            "kbd" => TagId::Kbd,
            "samp" => TagId::Samp,
            "var" => TagId::Var,
            "mark" => TagId::Mark,
            "small" => TagId::Small,
            "sub" => TagId::Sub,
            "sup" => TagId::Sup,
            "br" => TagId::Br,
            "img" => TagId::Img,
            "figure" => TagId::Figure,
            "figcaption" => TagId::Figcaption,
            "video" => TagId::Video,
            "audio" => TagId::Audio,
            "form" => TagId::Form,
            "input" => TagId::Input,
            "button" => TagId::Button,
            "select" => TagId::Select,
            "textarea" => TagId::Textarea,
            "label" => TagId::Label,
            "script" => TagId::Script,
            "style" => TagId::Style,
            "noscript" => TagId::Noscript,
            "iframe" => TagId::Iframe,
            "meta" => TagId::Meta,
            "link" => TagId::Link,
            "title" => TagId::Title,
            _ => TagId::Other,
        }
    }

    /// Get the tag name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            TagId::Html => "html",
            TagId::Head => "head",
            TagId::Body => "body",
            TagId::Article => "article",
            TagId::Main => "main",
            TagId::Section => "section",
            TagId::Aside => "aside",
            TagId::Nav => "nav",
            TagId::Header => "header",
            TagId::Footer => "footer",
            TagId::Div => "div",
            TagId::P => "p",
            TagId::Pre => "pre",
            TagId::Blockquote => "blockquote",
            TagId::H1 => "h1",
            TagId::H2 => "h2",
            TagId::H3 => "h3",
            TagId::H4 => "h4",
            TagId::H5 => "h5",
            TagId::H6 => "h6",
            TagId::Ul => "ul",
            TagId::Ol => "ol",
            TagId::Li => "li",
            TagId::Table => "table",
            TagId::Thead => "thead",
            TagId::Tbody => "tbody",
            TagId::Tr => "tr",
            TagId::Th => "th",
            TagId::Td => "td",
            TagId::A => "a",
            TagId::Span => "span",
            TagId::Em => "em",
            TagId::Strong => "strong",
            TagId::B => "b",
            TagId::I => "i",
            TagId::U => "u",
            TagId::Code => "code",
            TagId::Kbd => "kbd",
            TagId::Samp => "samp",
            TagId::Var => "var",
            TagId::Mark => "mark",
            TagId::Small => "small",
            TagId::Sub => "sub",
            TagId::Sup => "sup",
            TagId::Br => "br",
            TagId::Img => "img",
            TagId::Figure => "figure",
            TagId::Figcaption => "figcaption",
            TagId::Video => "video",
            TagId::Audio => "audio",
            TagId::Form => "form",
            TagId::Input => "input",
            TagId::Button => "button",
            TagId::Select => "select",
            TagId::Textarea => "textarea",
            TagId::Label => "label",
            TagId::Script => "script",
            TagId::Style => "style",
            TagId::Noscript => "noscript",
            TagId::Iframe => "iframe",
            TagId::Meta => "meta",
            TagId::Link => "link",
            TagId::Title => "title",
            TagId::Other => "div", // fallback
        }
    }

    /// Check if this is a boilerplate tag (nav/footer/header/aside/form)
    pub fn is_boilerplate(&self) -> bool {
        matches!(
            self,
            TagId::Nav
                | TagId::Footer
                | TagId::Header
                | TagId::Aside
                | TagId::Form
                | TagId::Script
                | TagId::Style
                | TagId::Noscript
        )
    }

    /// Check if this is a semantic main content tag
    pub fn is_semantic_main(&self) -> bool {
        matches!(self, TagId::Article | TagId::Main)
    }

    /// Check if this is a code container
    pub fn is_code_container(&self) -> bool {
        matches!(self, TagId::Pre | TagId::Code)
    }

    /// Check if this is a heading
    pub fn is_heading(&self) -> bool {
        matches!(self, TagId::H1 | TagId::H2 | TagId::H3 | TagId::H4 | TagId::H5 | TagId::H6)
    }

    /// Check if this is a block element
    pub fn is_block(&self) -> bool {
        matches!(
            self,
            TagId::Div
                | TagId::P
                | TagId::Pre
                | TagId::Blockquote
                | TagId::Article
                | TagId::Main
                | TagId::Section
                | TagId::Aside
                | TagId::Nav
                | TagId::Header
                | TagId::Footer
                | TagId::H1
                | TagId::H2
                | TagId::H3
                | TagId::H4
                | TagId::H5
                | TagId::H6
                | TagId::Ul
                | TagId::Ol
                | TagId::Li
                | TagId::Table
                | TagId::Tr
                | TagId::Figure
        )
    }

    /// Check if this tag can be a candidate root
    pub fn is_candidate_tag(&self) -> bool {
        matches!(
            self,
            TagId::Article
                | TagId::Main
                | TagId::Section
                | TagId::Div
                | TagId::P
                | TagId::Pre
                | TagId::Td
                | TagId::Li
                | TagId::Blockquote
        )
    }
}

/// Node kind - element or text
#[derive(Debug, Clone)]
pub enum NodeKind {
    /// Document root
    Document,
    /// Element node with tag and attributes
    Element {
        tag: TagId,
        /// Original tag name (for Other tags)
        tag_name: Option<String>,
    },
    /// Text node
    Text,
    /// Comment node (ignored for extraction)
    Comment,
}

/// A node in the arena
#[derive(Debug, Clone)]
pub struct Node {
    /// The kind of node
    pub kind: NodeKind,
    /// Parent node (None for root)
    pub parent: Option<NodeId>,
    /// First child node
    pub first_child: Option<NodeId>,
    /// Last child node (for efficient appending)
    pub last_child: Option<NodeId>,
    /// Next sibling node
    pub next_sibling: Option<NodeId>,
    /// Previous sibling (for traversal)
    pub prev_sibling: Option<NodeId>,
    /// Depth in the tree (0 for root)
    pub depth: u32,
}

impl Node {
    /// Create a new document node
    pub fn document() -> Self {
        Self {
            kind: NodeKind::Document,
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            depth: 0,
        }
    }

    /// Create a new element node
    pub fn element(tag: TagId, tag_name: Option<String>) -> Self {
        Self {
            kind: NodeKind::Element { tag, tag_name },
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            depth: 0,
        }
    }

    /// Create a new text node
    pub fn text() -> Self {
        Self {
            kind: NodeKind::Text,
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            depth: 0,
        }
    }

    /// Create a new comment node
    pub fn comment() -> Self {
        Self {
            kind: NodeKind::Comment,
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            depth: 0,
        }
    }

    /// Get the tag ID if this is an element
    pub fn tag(&self) -> Option<TagId> {
        match &self.kind {
            NodeKind::Element { tag, .. } => Some(*tag),
            _ => None,
        }
    }

    /// Check if this is a text node
    pub fn is_text(&self) -> bool {
        matches!(self.kind, NodeKind::Text)
    }

    /// Check if this is an element node
    pub fn is_element(&self) -> bool {
        matches!(self.kind, NodeKind::Element { .. })
    }
}

/// Attribute storage
#[derive(Debug, Clone, Default)]
pub struct Attributes {
    /// id attribute
    pub id: Option<String>,
    /// class attribute (space-separated)
    pub class: Option<String>,
    /// role attribute
    pub role: Option<String>,
    /// href attribute (for links)
    pub href: Option<String>,
    /// src attribute (for images)
    pub src: Option<String>,
    /// alt attribute (for images)
    pub alt: Option<String>,
    /// name attribute (for meta tags)
    pub name: Option<String>,
    /// content attribute (for meta tags)
    pub content: Option<String>,
    /// property attribute (for meta tags, e.g., og:title)
    pub property: Option<String>,
}

impl Attributes {
    /// Check if class or id contains any of the given keywords (case-insensitive)
    pub fn contains_keyword(&self, keywords: &[&str]) -> bool {
        let id_lower = self.id.as_ref().map(|s| s.to_ascii_lowercase());
        let class_lower = self.class.as_ref().map(|s| s.to_ascii_lowercase());

        for kw in keywords {
            let kw_lower = kw.to_ascii_lowercase();
            if let Some(ref id) = id_lower {
                if id.contains(&kw_lower) {
                    return true;
                }
            }
            if let Some(ref class) = class_lower {
                if class.contains(&kw_lower) {
                    return true;
                }
            }
        }
        false
    }
}

/// The node arena - stores all nodes and their data
#[derive(Debug)]
pub struct Arena {
    /// All nodes
    pub nodes: Vec<Node>,
    /// Text content for text nodes, indexed by NodeId
    pub text_content: HashMap<NodeId, String>,
    /// Attributes for element nodes, indexed by NodeId
    pub attributes: HashMap<NodeId, Attributes>,
    /// Maximum depth in the tree
    pub max_depth: u32,
}

impl Arena {
    /// Create a new empty arena
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            text_content: HashMap::new(),
            attributes: HashMap::new(),
            max_depth: 0,
        }
    }

    /// Add a node to the arena and return its ID
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = self.nodes.len() as NodeId;
        if node.depth > self.max_depth {
            self.max_depth = node.depth;
        }
        self.nodes.push(node);
        id
    }

    /// Get a node by ID
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    /// Get a mutable node by ID
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id as usize)
    }

    /// Set text content for a text node
    pub fn set_text(&mut self, id: NodeId, text: String) {
        self.text_content.insert(id, text);
    }

    /// Get text content for a node
    pub fn get_text(&self, id: NodeId) -> Option<&str> {
        self.text_content.get(&id).map(|s| s.as_str())
    }

    /// Set attributes for an element node
    pub fn set_attributes(&mut self, id: NodeId, attrs: Attributes) {
        self.attributes.insert(id, attrs);
    }

    /// Get attributes for a node
    pub fn get_attributes(&self, id: NodeId) -> Option<&Attributes> {
        self.attributes.get(&id)
    }

    /// Append a child node to a parent
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        let parent_depth = self.nodes[parent_id as usize].depth;

        // Set child's parent and depth
        self.nodes[child_id as usize].parent = Some(parent_id);
        self.nodes[child_id as usize].depth = parent_depth + 1;

        if self.nodes[child_id as usize].depth > self.max_depth {
            self.max_depth = self.nodes[child_id as usize].depth;
        }

        // Link to existing children
        if let Some(last_child) = self.nodes[parent_id as usize].last_child {
            self.nodes[last_child as usize].next_sibling = Some(child_id);
            self.nodes[child_id as usize].prev_sibling = Some(last_child);
        } else {
            self.nodes[parent_id as usize].first_child = Some(child_id);
        }

        self.nodes[parent_id as usize].last_child = Some(child_id);
    }

    /// Iterate over children of a node
    pub fn children(&self, id: NodeId) -> ChildrenIter<'_> {
        ChildrenIter {
            arena: self,
            current: self.nodes.get(id as usize).and_then(|n| n.first_child),
        }
    }

    /// Iterate over all descendants of a node (depth-first)
    pub fn descendants(&self, id: NodeId) -> DescendantsIter<'_> {
        DescendantsIter { arena: self, stack: vec![id], root: id }
    }

    /// Get all ancestors of a node (from parent to root)
    pub fn ancestors(&self, id: NodeId) -> AncestorsIter<'_> {
        AncestorsIter {
            arena: self,
            current: self.nodes.get(id as usize).and_then(|n| n.parent),
        }
    }

    /// Find the body element
    pub fn find_body(&self) -> Option<NodeId> {
        for (i, node) in self.nodes.iter().enumerate() {
            if let NodeKind::Element { tag: TagId::Body, .. } = node.kind {
                return Some(i as NodeId);
            }
        }
        None
    }

    /// Find the head element
    pub fn find_head(&self) -> Option<NodeId> {
        for (i, node) in self.nodes.iter().enumerate() {
            if let NodeKind::Element { tag: TagId::Head, .. } = node.kind {
                return Some(i as NodeId);
            }
        }
        None
    }

    /// Collect all text content within a subtree
    pub fn collect_text(&self, id: NodeId) -> String {
        let mut result = String::new();
        self.collect_text_recursive(id, &mut result, false);
        result
    }

    /// Collect text content, with option to preserve whitespace (for pre/code)
    fn collect_text_recursive(&self, id: NodeId, result: &mut String, preserve_ws: bool) {
        let node = match self.get(id) {
            Some(n) => n,
            None => return,
        };

        match &node.kind {
            NodeKind::Text => {
                if let Some(text) = self.get_text(id) {
                    if preserve_ws {
                        result.push_str(text);
                    } else {
                        // Normalize whitespace
                        for word in text.split_whitespace() {
                            if !result.is_empty() && !result.ends_with(' ') {
                                result.push(' ');
                            }
                            result.push_str(word);
                        }
                    }
                }
            }
            NodeKind::Element { tag, .. } => {
                let new_preserve = preserve_ws || tag.is_code_container();

                for child_id in self.children(id) {
                    self.collect_text_recursive(child_id, result, new_preserve);
                }

                // Add newline after block elements
                if tag.is_block() && !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            NodeKind::Document => {
                for child_id in self.children(id) {
                    self.collect_text_recursive(child_id, result, preserve_ws);
                }
            }
            NodeKind::Comment => {}
        }
    }

    /// Count nodes in a subtree
    pub fn subtree_node_count(&self, id: NodeId) -> usize {
        let mut count = 1; // count self
        for child_id in self.children(id) {
            count += self.subtree_node_count(child_id);
        }
        count
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over children of a node
pub struct ChildrenIter<'a> {
    arena: &'a Arena,
    current: Option<NodeId>,
}

impl Iterator for ChildrenIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.current = self.arena.nodes.get(current as usize)?.next_sibling;
        Some(current)
    }
}

/// Iterator over descendants (depth-first)
pub struct DescendantsIter<'a> {
    arena: &'a Arena,
    stack: Vec<NodeId>,
    root: NodeId,
}

impl Iterator for DescendantsIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.stack.pop()?;

        // Skip root on first call but process its children
        if current == self.root {
            // Push children in reverse order so we visit them left-to-right
            let children: Vec<_> = self.arena.children(current).collect();
            for child in children.into_iter().rev() {
                self.stack.push(child);
            }
            return self.next();
        }

        // Push children in reverse order
        let children: Vec<_> = self.arena.children(current).collect();
        for child in children.into_iter().rev() {
            self.stack.push(child);
        }

        Some(current)
    }
}

/// Iterator over ancestors (parent to root)
pub struct AncestorsIter<'a> {
    arena: &'a Arena,
    current: Option<NodeId>,
}

impl Iterator for AncestorsIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.current = self.arena.nodes.get(current as usize)?.parent;
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_id_from_name() {
        assert_eq!(TagId::from_name("div"), TagId::Div);
        assert_eq!(TagId::from_name("DIV"), TagId::Div);
        assert_eq!(TagId::from_name("article"), TagId::Article);
        assert_eq!(TagId::from_name("unknown"), TagId::Other);
    }

    #[test]
    fn test_arena_append_child() {
        let mut arena = Arena::new();
        let root = arena.add_node(Node::document());
        let child1 = arena.add_node(Node::element(TagId::Div, None));
        let child2 = arena.add_node(Node::element(TagId::P, None));

        arena.append_child(root, child1);
        arena.append_child(root, child2);

        let children: Vec<_> = arena.children(root).collect();
        assert_eq!(children, vec![child1, child2]);
        assert_eq!(arena.get(child1).unwrap().depth, 1);
        assert_eq!(arena.get(child2).unwrap().depth, 1);
    }

    #[test]
    fn test_boilerplate_tags() {
        assert!(TagId::Nav.is_boilerplate());
        assert!(TagId::Footer.is_boilerplate());
        assert!(!TagId::Article.is_boilerplate());
        assert!(!TagId::Div.is_boilerplate());
    }
}
