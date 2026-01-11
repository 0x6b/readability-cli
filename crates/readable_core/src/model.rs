//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.5158,  // LogTextLenChars
    0.5942,  // LogTextLenBytes
    0.5383,  // LogWordLikeCount
    -0.0775,  // LogCjkCharCount
    -11.8846,  // WhitespaceRatio
    1.908,  // LineCount
    -1.4152,  // ShortLineRatio
    0.4858,  // AvgLineLen
    // Punctuation (8-15)
    -3.6399,  // DotQRatio
    13.2472,  // JpPunctRatio
    9.5525,  // CommaRatio
    -75.8148,  // QuoteRatio
    -3.3103,  // SentenceEndRatio
    -246.9581,  // ColonRatio
    -377.0555,  // SemicolonRatio
    12.7097,  // ParenRatio
    // Link/boilerplate (16-23)
    0.7237,  // LogATagCount
    -0.9769,  // LogLiCount
    1.2339,  // LogButtonInputFormCount
    -7.2792,  // LinkDensity
    0.1082,  // NavTagInSubtreeCount
    -2.4953,  // LogFormElementCount
    0.1607,  // LogScriptStyleCount
    0.0,  // InlineStyleCount
    // Code-ness (24-31)
    -1.4129,  // LogPreCount
    4.216,  // LogCodeCount
    -0.2048,  // LogCodeBlockTextLen
    -78.6133,  // InlineCodeRatio
    4.9237,  // BraceRatio
    -3597.0442,  // BacktickRatio
    -5.3,  // IndentLineRatio
    -20.3177,  // CamelCaseRatio
    // Structure (32-39)
    7.1675,  // DepthNorm
    -0.7294,  // LogSubtreeNodeCount
    -0.0349,  // LogPCount
    -0.1596,  // LogHCount
    -0.3241,  // LogImgCount
    0.8335,  // SemanticMainFlag
    2.2206,  // TagPrior
    0.7158,  // LogTableCount
    // Keywords (40-47)
    -2.2815,  // LogPosKwHits
    0.0,  // LogNegKwHits
    10.8258,  // PosMinusNeg
    0.039,  // HasContentClass
    0.763,  // HasArticleClass
    -1.6821,  // HasMainRole
    0.0,  // HasNavClass
    -7.5132,  // HasSidebarClass
    // TOC signature (48-55)
    1.0683,  // TocLike
    -12.8367,  // ListLinkInteraction
    0.0238,  // LogNestedListCount
    2.8609,  // ListItemLinkRatio
    -0.6002,  // ShortTextLinkRatio
    -0.0356,  // RepetitiveStructure
    0.1886,  // LogUlOlCount
    -0.165,  // ConsecutiveLinks
    // Position/context (56-63)
    -1.2961,  // RelativePosition
    -12.8042,  // DistanceFromBody
    0.6498,  // HasParentArticle
    -0.8939,  // HasParentMain
    2.7705,  // SiblingContentRatio
    -0.6747,  // ChildDiversity
    -2.4383,  // TextDensity
    -0.5767  // ContentToBoilerplateRatio
];

/// Model bias term
const BIAS: f32 = -8.7534;

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
