//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.5609,  // LogTextLenChars
    0.6359,  // LogTextLenBytes
    0.5504,  // LogWordLikeCount
    -0.1729,  // LogCjkCharCount
    -15.6399,  // WhitespaceRatio
    1.4543,  // LineCount
    -0.8066,  // ShortLineRatio
    0.6509,  // AvgLineLen
    // Punctuation (8-15)
    -4.2137,  // DotQRatio
    16.4702,  // JpPunctRatio
    1.707,  // CommaRatio
    -62.5782,  // QuoteRatio
    -3.8117,  // SentenceEndRatio
    -226.5034,  // ColonRatio
    -320.7565,  // SemicolonRatio
    11.2825,  // ParenRatio
    // Link/boilerplate (16-23)
    0.5734,  // LogATagCount
    -0.7979,  // LogLiCount
    1.3415,  // LogButtonInputFormCount
    -6.0556,  // LinkDensity
    -0.1992,  // NavTagInSubtreeCount
    -2.4555,  // LogFormElementCount
    -0.0407,  // LogScriptStyleCount
    0.0,  // InlineStyleCount
    // Code-ness (24-31)
    -1.4385,  // LogPreCount
    3.6902,  // LogCodeCount
    -0.1429,  // LogCodeBlockTextLen
    -73.784,  // InlineCodeRatio
    4.1636,  // BraceRatio
    -3782.7641,  // BacktickRatio
    -3.4089,  // IndentLineRatio
    -21.6949,  // CamelCaseRatio
    // Structure (32-39)
    7.0362,  // DepthNorm
    -0.7135,  // LogSubtreeNodeCount
    0.2271,  // LogPCount
    -0.3248,  // LogHCount
    -0.2905,  // LogImgCount
    0.6053,  // SemanticMainFlag
    2.7752,  // TagPrior
    0.9675,  // LogTableCount
    // Keywords (40-47)
    -2.2098,  // LogPosKwHits
    0.0,  // LogNegKwHits
    10.9415,  // PosMinusNeg
    0.0576,  // HasContentClass
    0.8179,  // HasArticleClass
    -1.6944,  // HasMainRole
    0.0,  // HasNavClass
    -7.4925,  // HasSidebarClass
    // TOC signature (48-55)
    0.9269,  // TocLike
    -11.327,  // ListLinkInteraction
    -0.1196,  // LogNestedListCount
    2.9076,  // ListItemLinkRatio
    -0.3203,  // ShortTextLinkRatio
    -0.1552,  // RepetitiveStructure
    0.3538,  // LogUlOlCount
    -0.1413,  // ConsecutiveLinks
    // Position/context (56-63)
    -1.0444,  // RelativePosition
    -12.3721,  // DistanceFromBody
    0.8795,  // HasParentArticle
    -0.7005,  // HasParentMain
    2.7099,  // SiblingContentRatio
    -0.7785,  // ChildDiversity
    -2.8908,  // TextDensity
    -0.5171  // ContentToBoilerplateRatio
];

/// Model bias term
const BIAS: f32 = -8.833;

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
