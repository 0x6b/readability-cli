//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.0042,  // LogTextLenChars
    -0.0021, // LogTextLenBytes
    0.0158,  // LogWordLikeCount
    0.1146,  // LogCjkCharCount
    4.1181,  // WhitespaceRatio
    0.4034,  // LineCount
    -0.2297, // ShortLineRatio
    2.2193,  // AvgLineLen
    // Punctuation (8-15)
    -3.5961,  // DotQRatio
    6.898,    // JpPunctRatio
    -4.6619,  // CommaRatio
    -8.7111,  // QuoteRatio
    -2.7743,  // SentenceEndRatio
    -34.2986, // ColonRatio
    -41.2924, // SemicolonRatio
    -5.6735,  // ParenRatio
    // Link/boilerplate (16-23)
    0.1366,  // LogATagCount
    -0.0548, // LogLiCount
    -0.1729, // LogButtonInputFormCount
    -0.6953, // LinkDensity
    0.4608,  // NavTagInSubtreeCount
    -0.4758, // LogFormElementCount
    -0.2122, // LogScriptStyleCount
    -0.7376, // HighLinkDensityFlag
    // Code-ness (24-31)
    0.5036,   // LogPreCount
    0.0856,   // LogCodeCount
    0.0072,   // LogCodeBlockTextLen
    -0.7976,  // InlineCodeRatio
    -5.8021,  // BraceRatio
    -78.1705, // BacktickRatio
    -2.3572,  // IndentLineRatio
    -2.5254,  // CamelCaseRatio
    // Structure (32-39)
    6.3426,  // DepthNorm
    -0.0203, // LogSubtreeNodeCount
    0.321,   // LogPCount
    0.3267,  // LogHCount
    -0.2026, // LogImgCount
    -0.3485, // SemanticMainFlag
    0.7665,  // TagPrior
    1.0856,  // LogTableCount
    // Keywords (40-47)
    -0.266,  // LogPosKwHits
    0.0,     // LogNegKwHits
    -0.0147, // PosMinusNeg
    0.0015,  // HasContentClass
    0.3629,  // HasArticleClass
    -0.93,   // HasMainRole
    0.0,     // HasNavClass
    -0.8375, // HasSidebarClass
    // TOC signature (48-55)
    -0.1706, // TocLike
    -3.0818, // ListLinkInteraction
    0.5628,  // LogNestedListCount
    0.087,   // ListItemLinkRatio
    -0.489,  // ShortTextLinkRatio
    -0.009,  // RepetitiveStructure
    0.0826,  // LogUlOlCount
    -0.2229, // ConsecutiveLinks
    // Position/context (56-63)
    -0.9538, // RelativePosition
    -7.1064, // DistanceFromBody
    0.4592,  // HasParentArticle
    -0.4572, // HasParentMain
    2.7577,  // SiblingContentRatio
    -0.4723, // ChildDiversity
    -0.6259, // TextDensity
    0.5581,  // ContentToBoilerplateRatio
    // Readability-style (64-71)
    0.2451,  // LogCommaCount
    -0.7586, // AvgParagraphLen
    1.5668,  // IsArticleTag
    2.2225,  // IsMainTag
    1.1215,  // IsSemanticContainer
    -1.0862, // ParagraphTextScore
    0.8843,  // CleanTextRatio
    -1.2723, // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    -0.6037, // ContentDensity
    0.0423,  // LogCleanTextLen
    0.2287,  // AvgParagraphLenStrict
    1.315,   // LongParagraphRatio
    -0.008,  // TextToMarkupRatio
    0.7095,  // HasMinParagraphs
    0.3192,  // DescendantDiversity
    0.9798,  // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -7.1345;

/// Compute logit score for a feature vector
pub fn score(features: &FeatureVector) -> f32 {
    let mut sum = BIAS;
    for (weight, value) in WEIGHTS.iter().zip(features.values.iter()) {
        sum += weight * value;
    }
    sum
}

/// Convert logit to probability
#[allow(dead_code)]
pub fn probability(logit: f32) -> f32 {
    1.0 / (1.0 + (-logit).exp())
}
