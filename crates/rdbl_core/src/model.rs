//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.4237,  // LogTextLenChars
    -0.834,  // LogTextLenBytes
    -1.3431, // LogWordLikeCount
    0.0295,  // LogCjkCharCount
    -1.1312, // WhitespaceRatio
    0.274,   // LineCount
    -0.3221, // ShortLineRatio
    -0.2099, // AvgLineLen
    // Punctuation (8-15)
    -1.9578,   // DotQRatio
    14.7447,   // JpPunctRatio
    4.4319,    // CommaRatio
    -22.8995,  // QuoteRatio
    -0.5748,   // SentenceEndRatio
    -44.6691,  // ColonRatio
    -257.9356, // SemicolonRatio
    -179.9041, // ParenRatio
    // Link/boilerplate (16-23)
    0.0241,  // LogATagCount
    -0.2585, // LogLiCount
    0.1975,  // LogButtonInputFormCount
    2.9257,  // LinkDensity
    0.0816,  // NavTagInSubtreeCount
    -0.5699, // LogFormElementCount
    0.0773,  // LogScriptStyleCount
    0.0346,  // HighLinkDensityFlag
    // Code-ness (24-31)
    -0.1364,  // LogPreCount
    0.948,    // LogCodeCount
    -0.0249,  // LogCodeBlockTextLen
    -36.7737, // InlineCodeRatio
    180.1158, // BraceRatio
    9.2402,   // BacktickRatio
    -0.7636,  // IndentLineRatio
    -5.8335,  // CamelCaseRatio
    // Structure (32-39)
    2.6626,  // DepthNorm
    0.2461,  // LogSubtreeNodeCount
    0.0942,  // LogPCount
    -0.127,  // LogHCount
    -0.1118, // LogImgCount
    0.4656,  // SemanticMainFlag
    -0.2681, // TagPrior
    -0.0436, // LogTableCount
    // Keywords (40-47)
    -1.7034, // LogPosKwHits
    0.0,     // LogNegKwHits
    8.1266,  // PosMinusNeg
    0.1169,  // HasContentClass
    0.0801,  // HasArticleClass
    -0.4348, // HasMainRole
    0.0,     // HasNavClass
    0.1085,  // HasSidebarClass
    // TOC signature (48-55)
    1.072,   // TocLike
    -2.5953, // ListLinkInteraction
    0.0543,  // LogNestedListCount
    0.2294,  // ListItemLinkRatio
    0.0816,  // ShortTextLinkRatio
    -0.0987, // RepetitiveStructure
    0.0615,  // LogUlOlCount
    -0.051,  // ConsecutiveLinks
    // Position/context (56-63)
    -0.1319, // RelativePosition
    -3.3748, // DistanceFromBody
    0.7104,  // HasParentArticle
    -0.0233, // HasParentMain
    0.9157,  // SiblingContentRatio
    -0.1382, // ChildDiversity
    0.5391,  // TextDensity
    -0.6507, // ContentToBoilerplateRatio
    // Readability-style (64-71)
    -0.0217, // LogCommaCount
    0.4554,  // AvgParagraphLen
    1.0633,  // IsArticleTag
    0.4918,  // IsMainTag
    0.1228,  // IsSemanticContainer
    -0.1831, // ParagraphTextScore
    3.3873,  // CleanTextRatio
    -2.0245, // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    0.1967,  // ContentDensity
    1.9579,  // LogCleanTextLen
    0.1796,  // AvgParagraphLenStrict
    0.1017,  // LongParagraphRatio
    1.0243,  // TextToMarkupRatio
    -0.7923, // HasMinParagraphs
    1.4713,  // DescendantDiversity
    -0.1962, // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -9.7954;

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
