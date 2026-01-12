//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.376,   // LogTextLenChars
    0.6705,  // LogTextLenBytes
    -0.7906, // LogWordLikeCount
    -0.1768, // LogCjkCharCount
    1.1326,  // WhitespaceRatio
    0.9021,  // LineCount
    -0.2801, // ShortLineRatio
    0.3449,  // AvgLineLen
    // Punctuation (8-15)
    -1.5177,   // DotQRatio
    22.8947,   // JpPunctRatio
    -26.0379,  // CommaRatio
    -57.8779,  // QuoteRatio
    0.5396,    // SentenceEndRatio
    -70.2812,  // ColonRatio
    -206.0746, // SemicolonRatio
    -10.3624,  // ParenRatio
    // Link/boilerplate (16-23)
    0.133,   // LogATagCount
    -0.9379, // LogLiCount
    0.3198,  // LogButtonInputFormCount
    -3.9794, // LinkDensity
    -0.2861, // NavTagInSubtreeCount
    -1.1122, // LogFormElementCount
    0.2839,  // LogScriptStyleCount
    -0.1059, // HighLinkDensityFlag
    // Code-ness (24-31)
    -0.8463,  // LogPreCount
    1.2297,   // LogCodeCount
    0.1008,   // LogCodeBlockTextLen
    -27.8784, // InlineCodeRatio
    -3.0854,  // BraceRatio
    -197.467, // BacktickRatio
    -3.4853,  // IndentLineRatio
    -13.6671, // CamelCaseRatio
    // Structure (32-39)
    6.2345,  // DepthNorm
    -0.1243, // LogSubtreeNodeCount
    0.2762,  // LogPCount
    -0.0484, // LogHCount
    -0.2464, // LogImgCount
    0.2267,  // SemanticMainFlag
    -2.4899, // TagPrior
    0.1575,  // LogTableCount
    // Keywords (40-47)
    -1.182,  // LogPosKwHits
    0.0,     // LogNegKwHits
    7.0143,  // PosMinusNeg
    0.2412,  // HasContentClass
    0.1517,  // HasArticleClass
    -0.7994, // HasMainRole
    0.0,     // HasNavClass
    -5.3134, // HasSidebarClass
    // TOC signature (48-55)
    1.7397,  // TocLike
    -5.7128, // ListLinkInteraction
    -0.0371, // LogNestedListCount
    0.8492,  // ListItemLinkRatio
    0.0602,  // ShortTextLinkRatio
    -0.1908, // RepetitiveStructure
    0.9012,  // LogUlOlCount
    -0.3444, // ConsecutiveLinks
    // Position/context (56-63)
    -0.7632, // RelativePosition
    -8.5531, // DistanceFromBody
    1.1362,  // HasParentArticle
    -0.3893, // HasParentMain
    1.9193,  // SiblingContentRatio
    -0.54,   // ChildDiversity
    -0.7492, // TextDensity
    -1.2989, // ContentToBoilerplateRatio
    // Readability-style (64-71)
    0.0943,  // LogCommaCount
    1.1133,  // AvgParagraphLen
    2.6556,  // IsArticleTag
    2.1281,  // IsMainTag
    0.2911,  // IsSemanticContainer
    -1.1944, // ParagraphTextScore
    2.5054,  // CleanTextRatio
    -2.4061, // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    0.5157,  // ContentDensity
    0.4482,  // LogCleanTextLen
    1.0965,  // AvgParagraphLenStrict
    -0.1081, // LongParagraphRatio
    1.0946,  // TextToMarkupRatio
    -0.8663, // HasMinParagraphs
    2.6065,  // DescendantDiversity
    0.1105,  // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -11.5634;

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
