//! Keyword definitions for content/boilerplate detection
//!
//! Defines positive keywords that suggest main content and negative keywords
//! that suggest boilerplate (navigation, ads, etc.)

use crate::dom::Attributes;

/// Positive keywords for class/id that suggest content
pub const POSITIVE_KEYWORDS: &[&str] = &[
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
pub const NEGATIVE_KEYWORDS: &[&str] = &[
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

/// Get the positive keywords list
pub fn positive_keywords() -> &'static [&'static str] {
    POSITIVE_KEYWORDS
}

/// Get the negative keywords list
pub fn negative_keywords() -> &'static [&'static str] {
    NEGATIVE_KEYWORDS
}

/// Check if attributes contain positive keywords
pub fn has_positive_keywords(attrs: Option<&Attributes>) -> bool {
    attrs.is_some_and(|a| a.contains_keyword(POSITIVE_KEYWORDS))
}

/// Check if attributes contain negative keywords
pub fn has_negative_keywords(attrs: Option<&Attributes>) -> bool {
    attrs.is_some_and(|a| a.contains_keyword(NEGATIVE_KEYWORDS))
}
