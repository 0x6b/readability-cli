//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.1194,  // LogTextLenChars
    -0.0475, // LogTextLenBytes
    0.3646,  // LogWordLikeCount
    0.529,   // LogCjkCharCount
    -1.022,  // WhitespaceRatio
    0.2275,  // LineCount
    -1.6231, // ShortLineRatio
    -1.7209, // AvgLineLen
    // Punctuation (8-15)
    -23.0217,  // DotQRatio
    38.6705,   // JpPunctRatio
    -105.709,  // CommaRatio
    -18.2197,  // QuoteRatio
    -18.1652,  // SentenceEndRatio
    -287.8379, // ColonRatio
    -65.336,   // SemicolonRatio
    -17.0769,  // ParenRatio
    // Link/boilerplate (16-23)
    1.3533,  // LogATagCount
    -0.1949, // LogLiCount
    0.5564,  // LogButtonInputFormCount
    -8.8969, // LinkDensity
    0.4747,  // NavTagInSubtreeCount
    -1.1584, // LogFormElementCount
    -0.7057, // LogScriptStyleCount
    -0.9467, // HighLinkDensityFlag
    // Code-ness (24-31)
    3.9492,   // LogPreCount
    0.0453,   // LogCodeCount
    -0.4876,  // LogCodeBlockTextLen
    -5.9397,  // InlineCodeRatio
    4.8269,   // BraceRatio
    -419.342, // BacktickRatio
    -3.2281,  // IndentLineRatio
    -11.6244, // CamelCaseRatio
    // Structure (32-39)
    11.6456, // DepthNorm
    0.1441,  // LogSubtreeNodeCount
    -0.5786, // LogPCount
    -0.3164, // LogHCount
    -0.4829, // LogImgCount
    0.3498,  // SemanticMainFlag
    -0.2024, // TagPrior
    0.3182,  // LogTableCount
    // Keywords (40-47)
    -1.3695, // LogPosKwHits
    0.0,     // LogNegKwHits
    8.3073,  // PosMinusNeg
    -0.0076, // HasContentClass
    -0.3015, // HasArticleClass
    -0.4067, // HasMainRole
    0.0,     // HasNavClass
    -6.4595, // HasSidebarClass
    // TOC signature (48-55)
    1.0551,  // TocLike
    1.9567,  // ListLinkInteraction
    0.2339,  // LogNestedListCount
    0.8727,  // ListItemLinkRatio
    -0.948,  // ShortTextLinkRatio
    -0.0416, // RepetitiveStructure
    -1.0496, // LogUlOlCount
    -0.9755, // ConsecutiveLinks
    // Position/context (56-63)
    0.0115,   // RelativePosition
    -12.4536, // DistanceFromBody
    1.4813,   // HasParentArticle
    -0.5362,  // HasParentMain
    2.6177,   // SiblingContentRatio
    -0.1278,  // ChildDiversity
    -3.0048,  // TextDensity
    -1.0559,  // ContentToBoilerplateRatio
    // Readability-style (64-71)
    1.2352,  // LogCommaCount
    0.3253,  // AvgParagraphLen
    2.446,   // IsArticleTag
    3.2538,  // IsMainTag
    -0.0202, // IsSemanticContainer
    0.14,    // ParagraphTextScore
    5.9434,  // CleanTextRatio
    -2.6125, // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    0.6607,  // ContentDensity
    0.4619,  // LogCleanTextLen
    2.1728,  // AvgParagraphLenStrict
    -0.3496, // LongParagraphRatio
    0.4159,  // TextToMarkupRatio
    -1.4704, // HasMinParagraphs
    4.0131,  // DescendantDiversity
    0.5186,  // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -16.2786;

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
