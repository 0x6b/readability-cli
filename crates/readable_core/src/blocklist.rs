//! Curated blocklist for removing common boilerplate elements
//!
//! Inspired by EasyList/uBlock Origin annoyances lists, but curated to ~200 generic rules.
//! These are structural patterns (class/id based) that appear across many sites.

use crate::dom::{Arena, NodeId, NodeKind, TagId};

/// Blocklist entry with pattern and behavior
struct BlockRule {
    /// Substring to match in class/id (lowercase)
    pattern: &'static str,
    /// Maximum text length to remove (0 = always remove regardless of text)
    max_text_len: usize,
}

impl BlockRule {
    #[allow(dead_code)]
    const fn new(pattern: &'static str, max_text_len: usize) -> Self {
        Self { pattern, max_text_len }
    }

    /// Always remove regardless of content
    const fn always(pattern: &'static str) -> Self {
        Self { pattern, max_text_len: 0 }
    }

    /// Remove only if text content is small
    const fn if_small(pattern: &'static str) -> Self {
        Self { pattern, max_text_len: 200 }
    }

    /// Remove if text content is medium-sized
    const fn if_medium(pattern: &'static str) -> Self {
        Self { pattern, max_text_len: 500 }
    }
}

/// Cookie/consent banners - almost always remove
const COOKIE_CONSENT: &[BlockRule] = &[
    BlockRule::always("cookie-banner"),
    BlockRule::always("cookie-notice"),
    BlockRule::always("cookie-consent"),
    BlockRule::always("cookie-popup"),
    BlockRule::always("cookie-modal"),
    BlockRule::always("cookie-warning"),
    BlockRule::always("cookie-policy"),
    BlockRule::always("cookiebanner"),
    BlockRule::always("cookienotice"),
    BlockRule::always("gdpr-banner"),
    BlockRule::always("gdpr-notice"),
    BlockRule::always("gdpr-consent"),
    BlockRule::always("gdpr-popup"),
    BlockRule::always("gdpr-modal"),
    BlockRule::always("privacy-banner"),
    BlockRule::always("privacy-notice"),
    BlockRule::always("privacy-consent"),
    BlockRule::always("consent-banner"),
    BlockRule::always("consent-modal"),
    BlockRule::always("consent-popup"),
    BlockRule::always("cc-banner"),
    BlockRule::always("cc-window"),
    BlockRule::if_small("cookie"),
    BlockRule::if_small("gdpr"),
    BlockRule::if_small("consent"),
];

/// Newsletter/subscription prompts
const NEWSLETTER: &[BlockRule] = &[
    BlockRule::if_medium("newsletter"),
    BlockRule::if_medium("subscribe"),
    BlockRule::if_medium("subscription"),
    BlockRule::if_medium("signup-prompt"),
    BlockRule::if_medium("sign-up-prompt"),
    BlockRule::if_medium("email-signup"),
    BlockRule::if_medium("email-capture"),
    BlockRule::if_medium("mailing-list"),
    BlockRule::if_medium("mailinglist"),
    BlockRule::if_small("newsletter-popup"),
    BlockRule::if_small("newsletter-modal"),
    BlockRule::if_small("subscribe-popup"),
    BlockRule::if_small("subscribe-modal"),
];

/// Social sharing widgets
const SOCIAL: &[BlockRule] = &[
    BlockRule::if_small("share-buttons"),
    BlockRule::if_small("share-icons"),
    BlockRule::if_small("share-links"),
    BlockRule::if_small("share-bar"),
    BlockRule::if_small("share-box"),
    BlockRule::if_small("share-widget"),
    BlockRule::if_small("sharing-buttons"),
    BlockRule::if_small("sharing-icons"),
    BlockRule::if_small("social-share"),
    BlockRule::if_small("social-buttons"),
    BlockRule::if_small("social-icons"),
    BlockRule::if_small("social-links"),
    BlockRule::if_small("social-bar"),
    BlockRule::if_small("social-widget"),
    BlockRule::if_small("addthis"),
    BlockRule::if_small("sharethis"),
    BlockRule::if_small("share-this"),
    BlockRule::if_small("fb-share"),
    BlockRule::if_small("twitter-share"),
    BlockRule::if_small("linkedin-share"),
    BlockRule::if_small("pinterest-share"),
    BlockRule::if_small("whatsapp-share"),
];

/// Promotional content
const PROMO: &[BlockRule] = &[
    BlockRule::if_medium("promo-banner"),
    BlockRule::if_medium("promo-box"),
    BlockRule::if_medium("promotion"),
    BlockRule::if_medium("promotional"),
    BlockRule::if_medium("cta-banner"),
    BlockRule::if_medium("cta-box"),
    BlockRule::if_medium("call-to-action"),
    BlockRule::if_medium("upsell"),
    BlockRule::if_medium("cross-sell"),
    BlockRule::if_medium("sponsored"),
    BlockRule::if_medium("partner-content"),
    BlockRule::if_medium("paid-content"),
    BlockRule::if_small("promo"),
];

/// Related/recommended content
const RELATED: &[BlockRule] = &[
    BlockRule::if_medium("related-posts"),
    BlockRule::if_medium("related-articles"),
    BlockRule::if_medium("related-content"),
    BlockRule::if_medium("related-stories"),
    BlockRule::if_medium("recommended-posts"),
    BlockRule::if_medium("recommended-articles"),
    BlockRule::if_medium("recommended-content"),
    BlockRule::if_medium("recommended-stories"),
    BlockRule::if_medium("more-stories"),
    BlockRule::if_medium("more-articles"),
    BlockRule::if_medium("more-posts"),
    BlockRule::if_medium("you-may-like"),
    BlockRule::if_medium("you-might-like"),
    BlockRule::if_medium("also-read"),
    BlockRule::if_medium("read-more"),
    BlockRule::if_medium("read-next"),
    BlockRule::if_medium("outbrain"),
    BlockRule::if_medium("taboola"),
    BlockRule::if_medium("mgid"),
    BlockRule::if_medium("revcontent"),
];

/// Comments sections
const COMMENTS: &[BlockRule] = &[
    BlockRule::if_medium("comments-section"),
    BlockRule::if_medium("comments-area"),
    BlockRule::if_medium("comments-container"),
    BlockRule::if_medium("comment-section"),
    BlockRule::if_medium("comment-area"),
    BlockRule::if_medium("comment-list"),
    BlockRule::if_medium("disqus"),
    BlockRule::if_medium("disqus_thread"),
    BlockRule::if_medium("fb-comments"),
    BlockRule::if_medium("facebook-comments"),
    BlockRule::if_medium("livefyre"),
    BlockRule::if_medium("spot-im"),
];

/// Advertisements
const ADS: &[BlockRule] = &[
    BlockRule::if_small("ad-container"),
    BlockRule::if_small("ad-wrapper"),
    BlockRule::if_small("ad-slot"),
    BlockRule::if_small("ad-unit"),
    BlockRule::if_small("ad-banner"),
    BlockRule::if_small("ad-box"),
    BlockRule::if_small("ad-placement"),
    BlockRule::if_small("ad-block"),
    BlockRule::if_small("ads-container"),
    BlockRule::if_small("ads-wrapper"),
    BlockRule::if_small("advertisement"),
    BlockRule::if_small("advertisment"),
    BlockRule::if_small("adsense"),
    BlockRule::if_small("adsbygoogle"),
    BlockRule::if_small("dfp-ad"),
    BlockRule::if_small("gpt-ad"),
    BlockRule::if_small("google-ad"),
    BlockRule::if_small("doubleclick"),
    BlockRule::if_small("sponsored-ad"),
];

/// Overlays and modals (non-cookie)
const OVERLAYS: &[BlockRule] = &[
    BlockRule::if_small("modal-overlay"),
    BlockRule::if_small("popup-overlay"),
    BlockRule::if_small("lightbox-overlay"),
    BlockRule::if_small("overlay-backdrop"),
    BlockRule::if_small("modal-backdrop"),
    BlockRule::if_small("interstitial"),
    BlockRule::if_small("paywall"),
    BlockRule::if_small("regwall"),
    BlockRule::if_small("registration-wall"),
    BlockRule::if_small("login-wall"),
    BlockRule::if_small("gate-content"),
    BlockRule::if_small("content-gate"),
];

/// Author bio / byline boxes (often redundant)
const AUTHOR_BIO: &[BlockRule] = &[
    BlockRule::if_medium("author-bio"),
    BlockRule::if_medium("author-box"),
    BlockRule::if_medium("author-info"),
    BlockRule::if_medium("about-author"),
    BlockRule::if_medium("writer-bio"),
    BlockRule::if_medium("contributor-bio"),
];

/// Print/email/save buttons
const UTILITY_BUTTONS: &[BlockRule] = &[
    BlockRule::if_small("print-button"),
    BlockRule::if_small("print-link"),
    BlockRule::if_small("email-button"),
    BlockRule::if_small("email-link"),
    BlockRule::if_small("save-button"),
    BlockRule::if_small("bookmark-button"),
    BlockRule::if_small("tools-bar"),
    BlockRule::if_small("utility-bar"),
    BlockRule::if_small("article-tools"),
    BlockRule::if_small("post-tools"),
];

/// Sticky elements that are usually UI chrome
const STICKY: &[BlockRule] = &[
    BlockRule::if_small("sticky-header"),
    BlockRule::if_small("sticky-footer"),
    BlockRule::if_small("sticky-nav"),
    BlockRule::if_small("sticky-sidebar"),
    BlockRule::if_small("sticky-ad"),
    BlockRule::if_small("fixed-header"),
    BlockRule::if_small("fixed-footer"),
    BlockRule::if_small("fixed-nav"),
    BlockRule::if_small("floating-bar"),
    BlockRule::if_small("floating-banner"),
];

/// All rule categories combined
const ALL_RULES: &[&[BlockRule]] = &[
    COOKIE_CONSENT,
    NEWSLETTER,
    SOCIAL,
    PROMO,
    RELATED,
    COMMENTS,
    ADS,
    OVERLAYS,
    AUTHOR_BIO,
    UTILITY_BUTTONS,
    STICKY,
];

/// Check if a node should be blocked based on its class/id attributes
pub fn should_block(arena: &Arena, node_id: NodeId) -> bool {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return false,
    };

    // Only check element nodes
    if !matches!(&node.kind, NodeKind::Element { .. }) {
        return false;
    }

    // Get class and id
    let attrs = match arena.get_attributes(node_id) {
        Some(a) => a,
        None => return false,
    };

    let class = attrs.class.as_deref().unwrap_or("");
    let id = attrs.id.as_deref().unwrap_or("");

    // Combine and lowercase for matching
    let combined = format!("{} {}", class, id).to_lowercase();

    if combined.is_empty() {
        return false;
    }

    // Check all rules
    for category in ALL_RULES {
        for rule in *category {
            if combined.contains(rule.pattern) {
                // Always remove if max_text_len is 0
                if rule.max_text_len == 0 {
                    return true;
                }

                // Otherwise check text length
                let text_len = arena.collect_text(node_id).chars().count();
                if text_len <= rule.max_text_len {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a node matches high-priority patterns that should always be removed
/// regardless of position in the tree (cookie banners, overlays, etc.)
pub fn is_always_remove(arena: &Arena, node_id: NodeId) -> bool {
    let attrs = match arena.get_attributes(node_id) {
        Some(a) => a,
        None => return false,
    };

    let class = attrs.class.as_deref().unwrap_or("");
    let id = attrs.id.as_deref().unwrap_or("");
    let combined = format!("{} {}", class, id).to_lowercase();

    if combined.is_empty() {
        return false;
    }

    // High-priority patterns - always remove
    const ALWAYS_REMOVE_PATTERNS: &[&str] = &[
        "cookie-banner",
        "cookie-notice",
        "cookie-consent",
        "cookie-popup",
        "cookie-modal",
        "gdpr-banner",
        "gdpr-notice",
        "gdpr-consent",
        "consent-banner",
        "consent-modal",
        "privacy-banner",
        "privacy-notice",
        "cc-banner",
        "cc-window",
        "interstitial",
        "paywall",
        "regwall",
    ];

    for pattern in ALWAYS_REMOVE_PATTERNS {
        if combined.contains(pattern) {
            return true;
        }
    }

    // Check for aria-label patterns
    if let Some(role) = &attrs.role {
        let role_lower = role.to_lowercase();
        if role_lower.contains("dialog") || role_lower.contains("alertdialog") {
            // Check if it's a cookie/consent dialog
            if combined.contains("cookie")
                || combined.contains("consent")
                || combined.contains("gdpr")
                || combined.contains("privacy")
            {
                return true;
            }
        }
    }

    false
}

/// Check common fixed/sticky positioning patterns that indicate UI chrome
pub fn is_fixed_chrome(arena: &Arena, node_id: NodeId) -> bool {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return false,
    };

    // Check tag - typically divs
    let tag = match &node.kind {
        NodeKind::Element { tag, .. } => *tag,
        _ => return false,
    };

    if !matches!(tag, TagId::Div | TagId::Section | TagId::Aside) {
        return false;
    }

    let attrs = match arena.get_attributes(node_id) {
        Some(a) => a,
        None => return false,
    };

    let class = attrs.class.as_deref().unwrap_or("").to_lowercase();
    let id = attrs.id.as_deref().unwrap_or("").to_lowercase();

    // Fixed/sticky patterns
    let is_fixed = class.contains("fixed")
        || class.contains("sticky")
        || class.contains("floating")
        || id.contains("fixed")
        || id.contains("sticky")
        || id.contains("floating");

    if !is_fixed {
        return false;
    }

    // Check it's not the main content area
    let has_content_signal = class.contains("content")
        || class.contains("article")
        || class.contains("post")
        || class.contains("entry")
        || id.contains("content")
        || id.contains("article")
        || id.contains("post")
        || id.contains("entry");

    if has_content_signal {
        return false;
    }

    // Small fixed elements are usually chrome
    let text_len = arena.collect_text(node_id).chars().count();
    text_len < 300
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_html;

    #[test]
    fn test_cookie_banner_blocked() {
        let html = r#"
        <html><body>
            <div class="cookie-banner">Accept cookies</div>
            <article><p>Main content</p></article>
        </body></html>
        "#;

        let arena = parse_html(html);

        // Find the cookie banner
        for id in 0..arena.nodes.len() {
            if let Some(attrs) = arena.get_attributes(id as NodeId) {
                if attrs.class.as_deref() == Some("cookie-banner") {
                    assert!(should_block(&arena, id as NodeId));
                    assert!(is_always_remove(&arena, id as NodeId));
                    return;
                }
            }
        }
        panic!("Cookie banner not found");
    }

    #[test]
    fn test_social_share_blocked() {
        let html = r#"
        <html><body>
            <div class="social-share">Share on Facebook</div>
            <article><p>Main content</p></article>
        </body></html>
        "#;

        let arena = parse_html(html);

        for id in 0..arena.nodes.len() {
            if let Some(attrs) = arena.get_attributes(id as NodeId) {
                if attrs.class.as_deref() == Some("social-share") {
                    assert!(should_block(&arena, id as NodeId));
                    return;
                }
            }
        }
        panic!("Social share not found");
    }

    #[test]
    fn test_large_content_not_blocked() {
        // Newsletter content with > 500 characters should NOT be blocked
        let html = r#"
        <html><body>
            <div class="newsletter">
                <p>This newsletter contains substantial content that should not be removed by the blocklist.
                The blocklist is designed to remove short promotional snippets, not actual content sections.</p>
                <p>When a newsletter section contains genuine article content with multiple paragraphs of
                meaningful text, it should be preserved. This helps ensure we don't accidentally remove
                important information from the page.</p>
                <p>The threshold of 500 characters is chosen to balance between removing promotional noise
                and keeping substantial content. Most promotional newsletter boxes contain only a headline
                and a call-to-action, totaling under 200 characters.</p>
                <p>By contrast, actual content sections like this one will easily exceed 500 characters
                and should be kept in the final output for readers to consume.</p>
            </div>
        </body></html>
        "#;

        let arena = parse_html(html);

        for id in 0..arena.nodes.len() {
            if let Some(attrs) = arena.get_attributes(id as NodeId) {
                if attrs.class.as_deref() == Some("newsletter") {
                    // Large content should not be blocked
                    assert!(!should_block(&arena, id as NodeId));
                    return;
                }
            }
        }
        panic!("Newsletter not found");
    }
}
