//! Feature extraction for candidate scoring
//!
//! Extracts ~64 features from candidate nodes for logistic regression scoring.

mod code_metrics;
mod paragraph;
mod structure;
mod text_metrics;

use code_metrics::count_code_metrics;
use paragraph::{compute_descendant_diversity, compute_paragraph_stats};
use structure::count_elements;
// Re-export for tests
#[cfg(test)]
use text_metrics::is_cjk;
use text_metrics::{count_punctuation, count_text_metrics};

use crate::{
    dom::{Arena, Attributes, NodeId, TagId},
    keywords::{negative_keywords, positive_keywords},
};

/// Number of features in the feature vector
pub const NUM_FEATURES: usize = 80;

/// Feature vector for a candidate
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub values: [f32; NUM_FEATURES],
}

impl Default for FeatureVector {
    fn default() -> Self {
        Self { values: [0.0; NUM_FEATURES] }
    }
}

impl FeatureVector {
    /// Get the values as a slice
    pub fn as_slice(&self) -> &[f32] {
        &self.values
    }

    /// Extract a log1p-encoded count value: exp(value) - 1
    pub fn get_count(&self, index: FeatureIndex) -> usize {
        (self.values[index as usize].exp() - 1.0) as usize
    }

    /// Get raw feature value by index
    pub fn get(&self, index: FeatureIndex) -> f32 {
        self.values[index as usize]
    }
}

/// Feature indices - keep these stable for model compatibility
#[repr(usize)]
pub enum FeatureIndex {
    // Text quantity & composition (0-7)
    LogTextLenChars = 0,
    LogTextLenBytes = 1,
    LogWordLikeCount = 2,
    LogCjkCharCount = 3,
    WhitespaceRatio = 4,
    LineCount = 5,
    ShortLineRatio = 6,
    AvgLineLen = 7,

    // Punctuation & sentence signals (8-15)
    DotQRatio = 8,
    JpPunctRatio = 9,
    CommaRatio = 10,
    QuoteRatio = 11,
    SentenceEndRatio = 12,
    ColonRatio = 13,
    SemicolonRatio = 14,
    ParenRatio = 15,

    // Link/boilerplate signals (16-23)
    LogATagCount = 16,
    LogLiCount = 17,
    LogButtonInputFormCount = 18,
    LinkDensity = 19,
    NavTagInSubtreeCount = 20,
    LogFormElementCount = 21,
    LogScriptStyleCount = 22,
    HighLinkDensityFlag = 23, // 1.0 if link density > 0.25 (Readability.js threshold)

    // Code-ness features (24-31)
    LogPreCount = 24,
    LogCodeCount = 25,
    LogCodeBlockTextLen = 26,
    InlineCodeRatio = 27,
    BraceRatio = 28,
    BacktickRatio = 29,
    IndentLineRatio = 30,
    CamelCaseRatio = 31,

    // Structure & semantics (32-39)
    DepthNorm = 32,
    LogSubtreeNodeCount = 33,
    LogPCount = 34,
    LogHCount = 35,
    LogImgCount = 36,
    SemanticMainFlag = 37,
    TagPrior = 38,
    LogTableCount = 39,

    // Class/id keyword scores (40-47)
    LogPosKwHits = 40,
    LogNegKwHits = 41,
    PosMinusNeg = 42,
    HasContentClass = 43,
    HasArticleClass = 44,
    HasMainRole = 45,
    HasNavClass = 46,
    HasSidebarClass = 47,

    // TOC signature features (48-55)
    TocLike = 48,
    ListLinkInteraction = 49,
    LogNestedListCount = 50,
    ListItemLinkRatio = 51,
    ShortTextLinkRatio = 52,
    RepetitiveStructure = 53,
    LogUlOlCount = 54,
    ConsecutiveLinks = 55,

    // Position and context (56-63)
    RelativePosition = 56,
    DistanceFromBody = 57,
    HasParentArticle = 58,
    HasParentMain = 59,
    SiblingContentRatio = 60,
    ChildDiversity = 61,
    TextDensity = 62,
    ContentToBoilerplateRatio = 63,

    // Readability-style features (64-71)
    LogCommaCount = 64,        // Raw comma count (Readability.js core heuristic)
    AvgParagraphLen = 65,      // Average text length per paragraph
    IsArticleTag = 66,         // 1.0 if candidate is <article>
    IsMainTag = 67,            // 1.0 if candidate is <main>
    IsSemanticContainer = 68,  // 1.0 if article/main/section
    ParagraphTextScore = 69,   // p_count * log(avg_para_len) - Readability-like
    CleanTextRatio = 70,       // (text - link_text) / text
    AncestorArticleDepth = 71, // Distance to nearest article ancestor (0 if none)

    // Anti-over-extraction features (72-79)
    ContentDensity = 72, // text_len / subtree_node_count - high = focused content
    LogCleanTextLen = 73, // log(text_len - link_text_len) - content without links
    AvgParagraphLenStrict = 74, // avg chars per <p>, stricter threshold
    LongParagraphRatio = 75, // ratio of paragraphs > 80 chars
    TextToMarkupRatio = 76, // text_len / (subtree_node_count * 10)
    HasMinParagraphs = 77, // 1.0 if >= 3 paragraphs with > 50 chars each
    DescendantDiversity = 78, // variety of content tags in subtree
    ContentClusterScore = 79, // paragraphs close together vs scattered
}

/// Extract features from a candidate node
pub fn extract_features(arena: &Arena, node_id: NodeId) -> FeatureVector {
    let mut features = FeatureVector::default();

    // Collect text from the subtree
    let text = arena.collect_text(node_id);
    let text_len_chars = text.chars().count();
    let text_len_bytes = text.len();

    // Count various text metrics
    let (word_count, cjk_count, whitespace_count) = count_text_metrics(&text);
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();
    let short_lines = lines.iter().filter(|l| l.chars().count() < 40).count();

    // Text quantity features
    features.values[FeatureIndex::LogTextLenChars as usize] = log1p(text_len_chars as f32);
    features.values[FeatureIndex::LogTextLenBytes as usize] = log1p(text_len_bytes as f32);
    features.values[FeatureIndex::LogWordLikeCount as usize] = log1p(word_count as f32);
    features.values[FeatureIndex::LogCjkCharCount as usize] = log1p(cjk_count as f32);
    features.values[FeatureIndex::WhitespaceRatio as usize] =
        clamp01(whitespace_count as f32 / text_len_chars.max(1) as f32);
    features.values[FeatureIndex::LineCount as usize] = log1p(line_count as f32);
    features.values[FeatureIndex::ShortLineRatio as usize] =
        clamp01(short_lines as f32 / line_count.max(1) as f32);
    features.values[FeatureIndex::AvgLineLen as usize] =
        (text_len_chars as f32 / line_count.max(1) as f32).min(500.0) / 500.0;

    // Punctuation features
    let (dots, jp_punct, commas, quotes, colons, semicolons, parens) = count_punctuation(&text);
    let len_f = text_len_chars.max(1) as f32;
    features.values[FeatureIndex::DotQRatio as usize] = clamp01(dots as f32 / len_f);
    features.values[FeatureIndex::JpPunctRatio as usize] = clamp01(jp_punct as f32 / len_f);
    features.values[FeatureIndex::CommaRatio as usize] = clamp01(commas as f32 / len_f);
    features.values[FeatureIndex::QuoteRatio as usize] = clamp01(quotes as f32 / len_f);
    features.values[FeatureIndex::SentenceEndRatio as usize] =
        clamp01((dots + jp_punct) as f32 / len_f);
    features.values[FeatureIndex::ColonRatio as usize] = clamp01(colons as f32 / len_f);
    features.values[FeatureIndex::SemicolonRatio as usize] = clamp01(semicolons as f32 / len_f);
    features.values[FeatureIndex::ParenRatio as usize] = clamp01(parens as f32 / len_f);

    // Count elements in subtree
    let counts = count_elements(arena, node_id);

    // Link/boilerplate features
    features.values[FeatureIndex::LogATagCount as usize] = log1p(counts.a as f32);
    features.values[FeatureIndex::LogLiCount as usize] = log1p(counts.li as f32);
    features.values[FeatureIndex::LogButtonInputFormCount as usize] =
        log1p((counts.button + counts.input + counts.form) as f32);
    features.values[FeatureIndex::LinkDensity as usize] =
        clamp01(counts.link_text_len as f32 / text_len_chars.max(1) as f32);
    features.values[FeatureIndex::NavTagInSubtreeCount as usize] =
        log1p((counts.nav + counts.aside + counts.header + counts.footer) as f32);
    features.values[FeatureIndex::LogFormElementCount as usize] =
        log1p((counts.form + counts.input + counts.select + counts.textarea) as f32);
    features.values[FeatureIndex::LogScriptStyleCount as usize] =
        log1p((counts.script + counts.style) as f32);
    // High link density flag: 1.0 if link density > 0.25 (Readability.js threshold)
    let link_density_val = counts.link_text_len as f32 / text_len_chars.max(1) as f32;
    features.values[FeatureIndex::HighLinkDensityFlag as usize] =
        if link_density_val > 0.25 { 1.0 } else { 0.0 };

    // Code-ness features
    features.values[FeatureIndex::LogPreCount as usize] = log1p(counts.pre as f32);
    features.values[FeatureIndex::LogCodeCount as usize] = log1p(counts.code as f32);
    features.values[FeatureIndex::LogCodeBlockTextLen as usize] =
        log1p(counts.code_block_text_len as f32);
    features.values[FeatureIndex::InlineCodeRatio as usize] =
        clamp01(counts.inline_code_text_len as f32 / text_len_chars.max(1) as f32);

    let (braces, backticks, indent_lines, camel_case) = count_code_metrics(&text);
    features.values[FeatureIndex::BraceRatio as usize] = clamp01(braces as f32 / len_f);
    features.values[FeatureIndex::BacktickRatio as usize] = clamp01(backticks as f32 / len_f);
    features.values[FeatureIndex::IndentLineRatio as usize] =
        clamp01(indent_lines as f32 / line_count.max(1) as f32);
    features.values[FeatureIndex::CamelCaseRatio as usize] =
        clamp01(camel_case as f32 / word_count.max(1) as f32);

    // Structure features
    let node = arena.get(node_id);
    let depth = node.map(|n| n.depth).unwrap_or(0);
    let max_depth = arena.max_depth.max(1);

    features.values[FeatureIndex::DepthNorm as usize] = clamp01(depth as f32 / max_depth as f32);
    features.values[FeatureIndex::LogSubtreeNodeCount as usize] =
        log1p(arena.subtree_node_count(node_id) as f32);
    features.values[FeatureIndex::LogPCount as usize] = log1p(counts.p as f32);
    features.values[FeatureIndex::LogHCount as usize] = log1p(counts.h as f32);
    features.values[FeatureIndex::LogImgCount as usize] = log1p(counts.img as f32);
    features.values[FeatureIndex::LogTableCount as usize] = log1p(counts.table as f32);

    // Semantic features
    let is_semantic = is_semantic_main(arena, node_id);
    features.values[FeatureIndex::SemanticMainFlag as usize] = if is_semantic { 1.0 } else { 0.0 };
    features.values[FeatureIndex::TagPrior as usize] = get_tag_prior(arena, node_id);

    // Keyword features
    let (pos_hits, neg_hits) = count_keyword_hits(arena, node_id);
    features.values[FeatureIndex::LogPosKwHits as usize] = log1p(pos_hits as f32);
    features.values[FeatureIndex::LogNegKwHits as usize] = log1p(neg_hits as f32);
    features.values[FeatureIndex::PosMinusNeg as usize] =
        ((pos_hits as f32 - neg_hits as f32) / 10.0).clamp(-1.0, 1.0);

    let attrs = arena.get_attributes(node_id);
    features.values[FeatureIndex::HasContentClass as usize] =
        if has_keyword_in_attrs(attrs, &["content", "body"]) { 1.0 } else { 0.0 };
    features.values[FeatureIndex::HasArticleClass as usize] =
        if has_keyword_in_attrs(attrs, &["article", "post", "entry"]) { 1.0 } else { 0.0 };
    features.values[FeatureIndex::HasMainRole as usize] = if attrs
        .map_or(false, |a| a.role.as_ref().map_or(false, |r| r.eq_ignore_ascii_case("main")))
    {
        1.0
    } else {
        0.0
    };
    features.values[FeatureIndex::HasNavClass as usize] =
        if has_keyword_in_attrs(attrs, &["nav", "navigation", "menu"]) { 1.0 } else { 0.0 };
    features.values[FeatureIndex::HasSidebarClass as usize] =
        if has_keyword_in_attrs(attrs, &["sidebar", "side"]) { 1.0 } else { 0.0 };

    // TOC signature features
    let link_density = features.values[FeatureIndex::LinkDensity as usize];
    let short_line_ratio = features.values[FeatureIndex::ShortLineRatio as usize];
    let li_count = counts.li as f32;

    features.values[FeatureIndex::TocLike as usize] =
        link_density * short_line_ratio * log1p(li_count);
    features.values[FeatureIndex::ListLinkInteraction as usize] =
        link_density * clamp01(li_count / 50.0);
    features.values[FeatureIndex::LogNestedListCount as usize] = log1p(counts.nested_lists as f32);
    features.values[FeatureIndex::ListItemLinkRatio as usize] =
        clamp01(counts.li_with_link as f32 / counts.li.max(1) as f32);
    features.values[FeatureIndex::ShortTextLinkRatio as usize] =
        clamp01(counts.short_text_links as f32 / counts.a.max(1) as f32);
    features.values[FeatureIndex::RepetitiveStructure as usize] =
        if counts.li > 5 && counts.li_with_link as f32 / counts.li as f32 > 0.8 {
            1.0
        } else {
            0.0
        };
    features.values[FeatureIndex::LogUlOlCount as usize] = log1p((counts.ul + counts.ol) as f32);
    features.values[FeatureIndex::ConsecutiveLinks as usize] =
        log1p(counts.consecutive_links as f32);

    // Position and context features
    features.values[FeatureIndex::RelativePosition as usize] =
        compute_relative_position(arena, node_id);
    features.values[FeatureIndex::DistanceFromBody as usize] = clamp01(depth as f32 / 20.0);
    features.values[FeatureIndex::HasParentArticle as usize] =
        if has_ancestor_tag(arena, node_id, TagId::Article) { 1.0 } else { 0.0 };
    features.values[FeatureIndex::HasParentMain as usize] =
        if has_ancestor_tag(arena, node_id, TagId::Main) { 1.0 } else { 0.0 };
    features.values[FeatureIndex::SiblingContentRatio as usize] =
        compute_sibling_content_ratio(arena, node_id, text_len_chars);
    features.values[FeatureIndex::ChildDiversity as usize] =
        compute_child_diversity(arena, node_id);
    features.values[FeatureIndex::TextDensity as usize] =
        clamp01(text_len_chars as f32 / arena.subtree_node_count(node_id).max(1) as f32 / 100.0);
    features.values[FeatureIndex::ContentToBoilerplateRatio as usize] = clamp01(
        text_len_chars as f32
            / (text_len_chars
                + counts.nav_text_len
                + counts.aside_text_len
                + counts.footer_text_len)
                .max(1) as f32,
    );

    // Readability-style features (64-71)

    // LogCommaCount: raw comma count (Readability.js core heuristic for prose detection)
    features.values[FeatureIndex::LogCommaCount as usize] = log1p(commas as f32);

    // AvgParagraphLen: average text per paragraph (normalized)
    let avg_para_len = if counts.p > 0 { text_len_chars as f32 / counts.p as f32 } else { 0.0 };
    features.values[FeatureIndex::AvgParagraphLen as usize] = (avg_para_len / 500.0).min(1.0);

    // IsArticleTag, IsMainTag, IsSemanticContainer: tag identity features
    let tag = node.and_then(|n| n.tag());
    let is_article = tag == Some(TagId::Article);
    let is_main = tag == Some(TagId::Main);
    let is_section = tag == Some(TagId::Section);
    features.values[FeatureIndex::IsArticleTag as usize] = if is_article { 1.0 } else { 0.0 };
    features.values[FeatureIndex::IsMainTag as usize] = if is_main { 1.0 } else { 0.0 };
    features.values[FeatureIndex::IsSemanticContainer as usize] =
        if is_article || is_main || is_section { 1.0 } else { 0.0 };

    // ParagraphTextScore: p_count * log(avg_para_len) - Readability-like composite
    features.values[FeatureIndex::ParagraphTextScore as usize] =
        if counts.p > 0 && avg_para_len > 0.0 {
            ((counts.p as f32).sqrt() * log1p(avg_para_len)).min(10.0) / 10.0
        } else {
            0.0
        };

    // CleanTextRatio: (text - link_text) / text
    let clean_text_len = text_len_chars.saturating_sub(counts.link_text_len);
    features.values[FeatureIndex::CleanTextRatio as usize] =
        clamp01(clean_text_len as f32 / text_len_chars.max(1) as f32);

    // AncestorArticleDepth: distance to nearest article/main ancestor
    features.values[FeatureIndex::AncestorArticleDepth as usize] =
        compute_ancestor_semantic_depth(arena, node_id);

    // Anti-over-extraction features (72-79)
    let subtree_nodes = arena.subtree_node_count(node_id).max(1);

    // ContentDensity: text per node - high means focused content
    let content_density = text_len_chars as f32 / subtree_nodes as f32;
    features.values[FeatureIndex::ContentDensity as usize] = (content_density / 50.0).min(1.0);

    // LogCleanTextLen: log of text without links
    features.values[FeatureIndex::LogCleanTextLen as usize] = log1p(clean_text_len as f32);

    // Compute paragraph statistics
    let para_stats = compute_paragraph_stats(arena, node_id);

    // AvgParagraphLenStrict: stricter average (0 if < 3 paragraphs)
    features.values[FeatureIndex::AvgParagraphLenStrict as usize] =
        if para_stats.count >= 3 { (para_stats.avg_len / 200.0).min(1.0) } else { 0.0 };

    // LongParagraphRatio: ratio of paragraphs > 80 chars
    features.values[FeatureIndex::LongParagraphRatio as usize] = if para_stats.count > 0 {
        para_stats.long_count as f32 / para_stats.count as f32
    } else {
        0.0
    };

    // TextToMarkupRatio: higher = more text-focused, less markup-heavy
    features.values[FeatureIndex::TextToMarkupRatio as usize] =
        (text_len_chars as f32 / (subtree_nodes * 10) as f32).min(1.0);

    // HasMinParagraphs: 1.0 if >= 3 paragraphs with > 50 chars each
    features.values[FeatureIndex::HasMinParagraphs as usize] =
        if para_stats.substantial_count >= 3 { 1.0 } else { 0.0 };

    // DescendantDiversity: variety of content-related tags
    features.values[FeatureIndex::DescendantDiversity as usize] =
        compute_descendant_diversity(arena, node_id);

    // ContentClusterScore: are paragraphs clustered together or scattered?
    features.values[FeatureIndex::ContentClusterScore as usize] = para_stats.cluster_score;

    features
}

/// Check if node is a semantic main container
fn is_semantic_main(arena: &Arena, node_id: NodeId) -> bool {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return false,
    };

    if node.tag().map_or(false, |t| t.is_semantic_main()) {
        return true;
    }

    // Check for role="main" or positive keywords
    if let Some(attrs) = arena.get_attributes(node_id) {
        if attrs.role.as_ref().map_or(false, |r| r.eq_ignore_ascii_case("main")) {
            return true;
        }
        if attrs.contains_keyword(positive_keywords()) {
            return true;
        }
    }

    false
}

/// Get tag prior value
fn get_tag_prior(arena: &Arena, node_id: NodeId) -> f32 {
    match arena.get(node_id).and_then(|n| n.tag()) {
        Some(tag) => match tag {
            TagId::Article => 1.0,
            TagId::Main => 0.9,
            TagId::Section => 0.5,
            TagId::Div => 0.3,
            TagId::Pre => 0.6,
            TagId::P => 0.4,
            TagId::Td => 0.2,
            TagId::Li => 0.1,
            TagId::Blockquote => 0.5,
            _ => 0.2,
        },
        None => 0.0,
    }
}

/// Count keyword hits in attributes
fn count_keyword_hits(arena: &Arena, node_id: NodeId) -> (usize, usize) {
    let mut pos = 0;
    let mut neg = 0;

    if let Some(attrs) = arena.get_attributes(node_id) {
        for kw in positive_keywords() {
            if attrs.contains_keyword(&[kw]) {
                pos += 1;
            }
        }
        for kw in negative_keywords() {
            if attrs.contains_keyword(&[kw]) {
                neg += 1;
            }
        }
    }

    (pos, neg)
}

/// Check if attributes contain specific keywords
fn has_keyword_in_attrs(attrs: Option<&Attributes>, keywords: &[&str]) -> bool {
    attrs.map_or(false, |a| a.contains_keyword(keywords))
}

/// Check if node has ancestor with specific tag
fn has_ancestor_tag(arena: &Arena, node_id: NodeId, tag: TagId) -> bool {
    arena
        .ancestors(node_id)
        .any(|aid| arena.get(aid).and_then(|n| n.tag()) == Some(tag))
}

/// Compute normalized distance to nearest semantic ancestor (article/main)
/// Returns 0.0 if node is itself article/main, higher values for deeper nesting
fn compute_ancestor_semantic_depth(arena: &Arena, node_id: NodeId) -> f32 {
    let mut depth = 0;
    for ancestor_id in arena.ancestors(node_id) {
        depth += 1;
        if let Some(tag) = arena.get(ancestor_id).and_then(|n| n.tag()) {
            if matches!(tag, TagId::Article | TagId::Main) {
                // Found semantic container - normalize by depth
                return clamp01(depth as f32 / 10.0);
            }
        }
    }
    // No semantic ancestor found
    0.0
}

/// Compute relative position (0 = start, 1 = end of body)
fn compute_relative_position(arena: &Arena, node_id: NodeId) -> f32 {
    let body_id = match arena.find_body() {
        Some(id) => id,
        None => return 0.5,
    };

    let total_nodes = arena.subtree_node_count(body_id);
    if total_nodes == 0 {
        return 0.5;
    }

    // Count nodes before this one (rough position estimate)
    let mut count_before = 0;
    for desc_id in arena.descendants(body_id) {
        if desc_id == node_id {
            break;
        }
        count_before += 1;
    }

    clamp01(count_before as f32 / total_nodes as f32)
}

/// Compute sibling content ratio
fn compute_sibling_content_ratio(arena: &Arena, node_id: NodeId, self_text_len: usize) -> f32 {
    let parent_id = match arena.get(node_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return 1.0,
    };

    let mut sibling_text_len = 0;
    for sibling_id in arena.children(parent_id) {
        if sibling_id != node_id {
            sibling_text_len += arena.collect_text(sibling_id).chars().count();
        }
    }

    let total = self_text_len + sibling_text_len;
    if total == 0 {
        return 1.0;
    }

    clamp01(self_text_len as f32 / total as f32)
}

/// Compute child diversity (variety of child tags)
fn compute_child_diversity(arena: &Arena, node_id: NodeId) -> f32 {
    let mut tag_set = std::collections::HashSet::new();
    let mut child_count = 0;

    for (_, tag) in arena.child_elements(node_id) {
        tag_set.insert(tag);
        child_count += 1;
    }

    if child_count == 0 {
        return 0.0;
    }

    clamp01(tag_set.len() as f32 / child_count.min(10) as f32)
}

/// Natural log of (1 + x)
fn log1p(x: f32) -> f32 {
    (1.0 + x).ln()
}

/// Clamp value to [0, 1]
fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_html;

    #[test]
    fn test_extract_features_basic() {
        let html = r#"
        <html>
        <body>
            <article id="content">
                <h1>Title</h1>
                <p>This is a paragraph with some content.</p>
                <p>Another paragraph here.</p>
            </article>
        </body>
        </html>
        "#;

        let arena = parse_html(html);

        // Find the article
        let article_id = arena
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.tag() == Some(TagId::Article))
            .map(|(i, _)| i as NodeId);

        let features = extract_features(&arena, article_id.unwrap());

        // Check that we have reasonable features
        assert!(features.values[FeatureIndex::LogTextLenChars as usize] > 0.0);
        assert!(features.values[FeatureIndex::SemanticMainFlag as usize] > 0.0);
        assert!(features.values[FeatureIndex::LogPCount as usize] > 0.0);
    }

    #[test]
    fn test_cjk_detection() {
        assert!(is_cjk('日'));
        assert!(is_cjk('本'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('ア'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('1'));
    }

    #[test]
    fn test_code_features() {
        let html = r#"
        <html>
        <body>
            <pre><code>function foo() {
    return bar;
}</code></pre>
        </body>
        </html>
        "#;

        let arena = parse_html(html);
        let body_id = arena.find_body().unwrap();

        // Find the pre element
        let pre_id = arena
            .descendants(body_id)
            .find(|&id| arena.get(id).and_then(|n| n.tag()) == Some(TagId::Pre));

        let features = extract_features(&arena, pre_id.unwrap());

        // Should have code-related features
        assert!(features.values[FeatureIndex::LogPreCount as usize] > 0.0);
        assert!(features.values[FeatureIndex::LogCodeBlockTextLen as usize] > 0.0);
    }
}
