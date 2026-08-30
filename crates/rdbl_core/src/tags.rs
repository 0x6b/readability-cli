//! Tag group utilities for consistent tag classification
//!
//! Centralizes tag group definitions to avoid repeated matches!() blocks.

use crate::dom::TagId;

/// Content container tags (article, main, section, div)
pub const CONTENT_CONTAINERS: &[TagId] = &[TagId::Article, TagId::Main, TagId::Section, TagId::Div];

/// Refinement container tags (div, section, article) - for drilling down
pub const REFINEMENT_CONTAINERS: &[TagId] = &[TagId::Div, TagId::Section, TagId::Article];

/// Semantic main content tags
pub const SEMANTIC_MAIN: &[TagId] = &[TagId::Article, TagId::Main];

/// Small content tags that may need ancestor fallback
pub const SMALL_CONTENT_TAGS: &[TagId] = &[TagId::P, TagId::Li, TagId::Td];

/// Media-related tags
pub const MEDIA_TAGS: &[TagId] = &[
    TagId::Img,
    TagId::Figure,
    TagId::Figcaption,
    TagId::Video,
    TagId::Audio,
    TagId::Iframe,
];

/// Self-closing (void) tags
pub const VOID_TAGS: &[TagId] = &[TagId::Br, TagId::Img, TagId::Input, TagId::Meta, TagId::Link];

/// Check if tag is a content container
#[inline]
pub fn is_content_container(tag: TagId) -> bool {
    CONTENT_CONTAINERS.contains(&tag)
}

/// Check if tag is a refinement container (for drilling into broad selections)
#[inline]
pub fn is_refinement_container(tag: TagId) -> bool {
    REFINEMENT_CONTAINERS.contains(&tag)
}

/// Check if tag is a semantic main container
#[inline]
pub fn is_semantic_main(tag: TagId) -> bool {
    SEMANTIC_MAIN.contains(&tag)
}

/// Check if tag is a small content tag that may need ancestor fallback
#[inline]
pub fn is_small_content_tag(tag: TagId) -> bool {
    SMALL_CONTENT_TAGS.contains(&tag)
}

/// Check if tag is media-related
#[inline]
pub fn is_media_tag(tag: TagId) -> bool {
    MEDIA_TAGS.contains(&tag)
}

/// Check if tag is self-closing (void)
#[inline]
pub fn is_void_tag(tag: TagId) -> bool {
    VOID_TAGS.contains(&tag)
}
