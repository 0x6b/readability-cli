//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.2049,  // LogTextLenChars
    0.385,   // LogTextLenBytes
    -0.7076, // LogWordLikeCount
    0.1499,  // LogCjkCharCount
    -6.9759, // WhitespaceRatio
    1.1356,  // LineCount
    -2.0952, // ShortLineRatio
    0.5481,  // AvgLineLen
    // Punctuation (8-15)
    1.0227,    // DotQRatio
    -43.4219,  // JpPunctRatio
    -64.0008,  // CommaRatio
    -67.846,   // QuoteRatio
    0.1516,    // SentenceEndRatio
    -231.4776, // ColonRatio
    -358.6015, // SemicolonRatio
    3.6518,    // ParenRatio
    // Link/boilerplate (16-23)
    0.3438,  // LogATagCount
    -0.793,  // LogLiCount
    0.6367,  // LogButtonInputFormCount
    -4.1648, // LinkDensity
    -0.506,  // NavTagInSubtreeCount
    -1.6665, // LogFormElementCount
    0.3423,  // LogScriptStyleCount
    0.1334,  // HighLinkDensityFlag
    // Code-ness (24-31)
    -2.2295,   // LogPreCount
    2.963,     // LogCodeCount
    0.1543,    // LogCodeBlockTextLen
    -81.8383,  // InlineCodeRatio
    8.2432,    // BraceRatio
    -5591.035, // BacktickRatio
    -0.4692,   // IndentLineRatio
    -22.0147,  // CamelCaseRatio
    // Structure (32-39)
    5.6879,  // DepthNorm
    -0.3172, // LogSubtreeNodeCount
    0.3125,  // LogPCount
    -0.2264, // LogHCount
    -0.3792, // LogImgCount
    0.242,   // SemanticMainFlag
    -4.0994, // TagPrior
    -0.0638, // LogTableCount
    // Keywords (40-47)
    -2.1757, // LogPosKwHits
    0.0,     // LogNegKwHits
    10.3886, // PosMinusNeg
    0.0523,  // HasContentClass
    0.264,   // HasArticleClass
    -1.2096, // HasMainRole
    0.0,     // HasNavClass
    -5.864,  // HasSidebarClass
    // TOC signature (48-55)
    2.8381,  // TocLike
    -9.6697, // ListLinkInteraction
    -0.0042, // LogNestedListCount
    1.3659,  // ListItemLinkRatio
    -0.606,  // ShortTextLinkRatio
    0.2144,  // RepetitiveStructure
    0.0502,  // LogUlOlCount
    0.024,   // ConsecutiveLinks
    // Position/context (56-63)
    -0.7666, // RelativePosition
    -9.4353, // DistanceFromBody
    0.7527,  // HasParentArticle
    0.0043,  // HasParentMain
    2.1935,  // SiblingContentRatio
    -0.6601, // ChildDiversity
    -1.6632, // TextDensity
    -2.3768, // ContentToBoilerplateRatio
    // Readability-style (64-71)
    0.9229,  // LogCommaCount
    0.8295,  // AvgParagraphLen
    4.444,   // IsArticleTag
    3.1598,  // IsMainTag
    0.7551,  // IsSemanticContainer
    -0.4336, // ParagraphTextScore
    4.639,   // CleanTextRatio
    -3.4818, // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    0.8962,  // ContentDensity
    0.311,   // LogCleanTextLen
    -0.1053, // AvgParagraphLenStrict
    0.1883,  // LongParagraphRatio
    0.5669,  // TextToMarkupRatio
    0.4338,  // HasMinParagraphs
    4.1662,  // DescendantDiversity
    -1.3586, // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -6.5116;

/// Compute logit score for a feature vector
pub fn score(features: &FeatureVector) -> f32 {
    let mut sum = BIAS;
    for i in 0..NUM_FEATURES {
        sum += WEIGHTS[i] * features.values[i];
    }
    sum
}

/// Convert logit to probability
#[allow(dead_code)]
pub fn probability(logit: f32) -> f32 {
    1.0 / (1.0 + (-logit).exp())
}
