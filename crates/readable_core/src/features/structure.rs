//! DOM structure metrics: element counting and structural analysis

use crate::dom::{Arena, NodeId, NodeKind, TagId};

/// Element counts for feature extraction
pub struct ElementCounts {
    pub a: usize,
    pub li: usize,
    pub button: usize,
    pub input: usize,
    pub form: usize,
    pub select: usize,
    pub textarea: usize,
    pub nav: usize,
    pub aside: usize,
    pub header: usize,
    pub footer: usize,
    pub script: usize,
    pub style: usize,
    pub pre: usize,
    pub code: usize,
    pub p: usize,
    pub h: usize,
    pub img: usize,
    pub table: usize,
    pub ul: usize,
    pub ol: usize,
    pub link_text_len: usize,
    pub code_block_text_len: usize,
    pub inline_code_text_len: usize,
    pub nested_lists: usize,
    pub li_with_link: usize,
    pub short_text_links: usize,
    pub consecutive_links: usize,
    pub nav_text_len: usize,
    pub aside_text_len: usize,
    pub footer_text_len: usize,
}

/// Count various elements in a subtree
pub fn count_elements(arena: &Arena, node_id: NodeId) -> ElementCounts {
    let mut counts = ElementCounts {
        a: 0,
        li: 0,
        button: 0,
        input: 0,
        form: 0,
        select: 0,
        textarea: 0,
        nav: 0,
        aside: 0,
        header: 0,
        footer: 0,
        script: 0,
        style: 0,
        pre: 0,
        code: 0,
        p: 0,
        h: 0,
        img: 0,
        table: 0,
        ul: 0,
        ol: 0,
        link_text_len: 0,
        code_block_text_len: 0,
        inline_code_text_len: 0,
        nested_lists: 0,
        li_with_link: 0,
        short_text_links: 0,
        consecutive_links: 0,
        nav_text_len: 0,
        aside_text_len: 0,
        footer_text_len: 0,
    };

    count_elements_recursive(arena, node_id, &mut counts, false, 0);
    counts
}

fn count_elements_recursive(
    arena: &Arena,
    node_id: NodeId,
    counts: &mut ElementCounts,
    in_pre: bool,
    list_depth: usize,
) {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return,
    };

    let mut current_in_pre = in_pre;
    let mut current_list_depth = list_depth;

    if let NodeKind::Element { tag, .. } = &node.kind {
        match tag {
            TagId::A => {
                counts.a += 1;
                let text = arena.collect_text(node_id);
                counts.link_text_len += text.chars().count();
                if text.chars().count() < 30 {
                    counts.short_text_links += 1;
                }
            }
            TagId::Li => {
                counts.li += 1;
                // Check if li contains a link
                if has_direct_link_child(arena, node_id) {
                    counts.li_with_link += 1;
                }
            }
            TagId::Button => counts.button += 1,
            TagId::Input => counts.input += 1,
            TagId::Form => counts.form += 1,
            TagId::Select => counts.select += 1,
            TagId::Textarea => counts.textarea += 1,
            TagId::Nav => {
                counts.nav += 1;
                counts.nav_text_len += arena.collect_text(node_id).chars().count();
            }
            TagId::Aside => {
                counts.aside += 1;
                counts.aside_text_len += arena.collect_text(node_id).chars().count();
            }
            TagId::Header => counts.header += 1,
            TagId::Footer => {
                counts.footer += 1;
                counts.footer_text_len += arena.collect_text(node_id).chars().count();
            }
            TagId::Script => counts.script += 1,
            TagId::Style => counts.style += 1,
            TagId::Pre => {
                counts.pre += 1;
                current_in_pre = true;
                counts.code_block_text_len += arena.collect_text(node_id).chars().count();
            }
            TagId::Code => {
                counts.code += 1;
                if in_pre {
                    // Already counted in pre
                } else {
                    counts.inline_code_text_len += arena.collect_text(node_id).chars().count();
                }
            }
            TagId::P => counts.p += 1,
            TagId::H1 | TagId::H2 | TagId::H3 | TagId::H4 | TagId::H5 | TagId::H6 => counts.h += 1,
            TagId::Img => counts.img += 1,
            TagId::Table => counts.table += 1,
            TagId::Ul | TagId::Ol => {
                if *tag == TagId::Ul {
                    counts.ul += 1;
                } else {
                    counts.ol += 1;
                }
                if list_depth > 0 {
                    counts.nested_lists += 1;
                }
                current_list_depth += 1;
            }
            _ => {}
        }
    }

    // Count consecutive links
    let mut prev_was_link = false;
    for child_id in arena.children(node_id) {
        if let Some(child) = arena.get(child_id) {
            if let NodeKind::Element { tag: TagId::A, .. } = &child.kind {
                if prev_was_link {
                    counts.consecutive_links += 1;
                }
                prev_was_link = true;
            } else {
                prev_was_link = false;
            }
        }
    }

    // Recurse
    for child_id in arena.children(node_id) {
        count_elements_recursive(arena, child_id, counts, current_in_pre, current_list_depth);
    }
}

/// Check if a node has a direct link child
fn has_direct_link_child(arena: &Arena, node_id: NodeId) -> bool {
    for child_id in arena.children(node_id) {
        if let Some(child) = arena.get(child_id) {
            if let NodeKind::Element { tag: TagId::A, .. } = &child.kind {
                return true;
            }
        }
    }
    false
}
