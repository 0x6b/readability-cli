//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.3122,  // LogTextLenChars
    0.3711,  // LogTextLenBytes
    0.363,   // LogWordLikeCount
    -0.0048, // LogCjkCharCount
    -2.9187, // WhitespaceRatio
    0.0331,  // LineCount
    0.5819,  // ShortLineRatio
    -1.4705, // AvgLineLen
    // Punctuation (8-15)
    -5.7879,  // DotQRatio
    -2.7901,  // JpPunctRatio
    41.981,   // CommaRatio
    -21.8169, // QuoteRatio
    -5.9968,  // SentenceEndRatio
    1.8525,   // ColonRatio
    57.1866,  // SemicolonRatio
    10.6739,  // ParenRatio
    // Link/boilerplate (16-23)
    -0.2038, // LogATagCount
    0.0744,  // LogLiCount
    -1.4773, // LogButtonInputFormCount
    -1.8386, // LinkDensity
    -0.0526, // NavTagInSubtreeCount
    0.411,   // LogFormElementCount
    -0.6471, // LogScriptStyleCount
    0.0,     // InlineStyleCount
    // Code-ness (24-31)
    0.6338,   // LogPreCount
    -0.1226,  // LogCodeCount
    -0.1591,  // LogCodeBlockTextLen
    10.6989,  // InlineCodeRatio
    -1.9357,  // BraceRatio
    850.7551, // BacktickRatio
    -1.0262,  // IndentLineRatio
    -0.9767,  // CamelCaseRatio
    // Structure (32-39)
    3.5531,  // DepthNorm
    -0.1533, // LogSubtreeNodeCount
    0.0624,  // LogPCount
    0.3421,  // LogHCount
    -0.1109, // LogImgCount
    1.7224,  // SemanticMainFlag
    0.5726,  // TagPrior
    0.0715,  // LogTableCount
    // Keywords (40-47)
    -2.0821, // LogPosKwHits
    0.0,     // LogNegKwHits
    12.1791, // PosMinusNeg
    0.9167,  // HasContentClass
    1.4625,  // HasArticleClass
    2.5418,  // HasMainRole
    0.0,     // HasNavClass
    0.0,     // HasSidebarClass
    // TOC signature (48-55)
    4.2467,  // TocLike
    -6.3583, // ListLinkInteraction
    -0.3597, // LogNestedListCount
    -1.4573, // ListItemLinkRatio
    -0.1574, // ShortTextLinkRatio
    -0.4458, // RepetitiveStructure
    -0.9057, // LogUlOlCount
    -0.461,  // ConsecutiveLinks
    // Position/context (56-63)
    -1.9928, // RelativePosition
    -6.7191, // DistanceFromBody
    -0.2945, // HasParentArticle
    -0.0406, // HasParentMain
    0.9065,  // SiblingContentRatio
    -0.8994, // ChildDiversity
    -0.0064, // TextDensity
    0.4808,  // ContentToBoilerplateRatio
];

/// Model bias term
const BIAS: f32 = -8.3458;

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
