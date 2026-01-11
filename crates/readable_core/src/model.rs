//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.2714,  // LogTextLenChars
    0.37,  // LogTextLenBytes
    -0.0988,  // LogWordLikeCount
    -0.2292,  // LogCjkCharCount
    -11.233,  // WhitespaceRatio
    1.3583,  // LineCount
    -1.4069,  // ShortLineRatio
    2.7467,  // AvgLineLen
    // Punctuation (8-15)
    10.943,  // DotQRatio
    27.8588,  // JpPunctRatio
    -16.195,  // CommaRatio
    -71.1657,  // QuoteRatio
    14.0433,  // SentenceEndRatio
    -238.4702,  // ColonRatio
    -151.9556,  // SemicolonRatio
    -8.9385,  // ParenRatio
    // Link/boilerplate (16-23)
    0.8512,  // LogATagCount
    -1.0129,  // LogLiCount
    4.0552,  // LogButtonInputFormCount
    -7.0446,  // LinkDensity
    -0.0699,  // NavTagInSubtreeCount
    -4.6433,  // LogFormElementCount
    0.1145,  // LogScriptStyleCount
    0.0,  // InlineStyleCount
    // Code-ness (24-31)
    -1.3793,  // LogPreCount
    2.6036,  // LogCodeCount
    -0.2306,  // LogCodeBlockTextLen
    -13.3056,  // InlineCodeRatio
    -10.8904,  // BraceRatio
    -974.8353,  // BacktickRatio
    -0.6501,  // IndentLineRatio
    -28.249,  // CamelCaseRatio
    // Structure (32-39)
    4.1605,  // DepthNorm
    0.2231,  // LogSubtreeNodeCount
    -0.0379,  // LogPCount
    0.1563,  // LogHCount
    -0.3236,  // LogImgCount
    -0.1593,  // SemanticMainFlag
    1.6692,  // TagPrior
    0.867,  // LogTableCount
    // Keywords (40-47)
    -0.7595,  // LogPosKwHits
    0.0,  // LogNegKwHits
    4.5934,  // PosMinusNeg
    0.0409,  // HasContentClass
    0.7725,  // HasArticleClass
    -2.2619,  // HasMainRole
    0.0,  // HasNavClass
    0.0,  // HasSidebarClass
    // TOC signature (48-55)
    -0.6821,  // TocLike
    -14.0593,  // ListLinkInteraction
    0.7928,  // LogNestedListCount
    3.6519,  // ListItemLinkRatio
    -0.703,  // ShortTextLinkRatio
    -0.0213,  // RepetitiveStructure
    -0.2995,  // LogUlOlCount
    -0.4696,  // ConsecutiveLinks
    // Position/context (56-63)
    -1.0158,  // RelativePosition
    -10.3144,  // DistanceFromBody
    0.249,  // HasParentArticle
    -0.713,  // HasParentMain
    2.8801,  // SiblingContentRatio
    -0.6248,  // ChildDiversity
    -1.9079,  // TextDensity
    -0.3086  // ContentToBoilerplateRatio
];

/// Model bias term
const BIAS: f32 = -4.3777;

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
