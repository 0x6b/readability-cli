//! Logistic regression model for content scoring
//!
//! Weights trained via tools/train_logreg.py on Mozilla Readability test data.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Model weights - trained via tools/train_logreg.py
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7)
    0.0113,  // LogTextLenChars
    0.0068,  // LogTextLenBytes
    0.0116,  // LogWordLikeCount
    0.0437,  // LogCjkCharCount
    2.1303,  // WhitespaceRatio
    0.1555,  // LineCount
    -0.1193,  // ShortLineRatio
    0.7986,  // AvgLineLen
    // Punctuation (8-15)
    -2.0428,  // DotQRatio
    0.0154,  // JpPunctRatio
    -2.6033,  // CommaRatio
    -3.2228,  // QuoteRatio
    -1.9166,  // SentenceEndRatio
    -14.6084,  // ColonRatio
    -24.2575,  // SemicolonRatio
    -2.732,  // ParenRatio
    // Link/boilerplate (16-23)
    0.0588,  // LogATagCount
    -0.0069,  // LogLiCount
    -0.0679,  // LogButtonInputFormCount
    -0.4184,  // LinkDensity
    0.3115,  // NavTagInSubtreeCount
    -0.1002,  // LogFormElementCount
    -0.1112,  // LogScriptStyleCount
    -0.4278,  // HighLinkDensityFlag
    // Code-ness (24-31)
    0.295,  // LogPreCount
    0.1226,  // LogCodeCount
    0.0452,  // LogCodeBlockTextLen
    -0.4391,  // InlineCodeRatio
    -2.7837,  // BraceRatio
    -35.3325,  // BacktickRatio
    -1.6105,  // IndentLineRatio
    -0.9662,  // CamelCaseRatio
    // Structure (32-39)
    3.3191,  // DepthNorm
    0.0516,  // LogSubtreeNodeCount
    0.1813,  // LogPCount
    0.2004,  // LogHCount
    -0.0272,  // LogImgCount
    -0.2116,  // SemanticMainFlag
    0.7674,  // TagPrior
    0.6288,  // LogTableCount
    // Keywords (40-47)
    -0.187,  // LogPosKwHits
    0.0,  // LogNegKwHits
    -0.3867,  // PosMinusNeg
    0.0415,  // HasContentClass
    0.0956,  // HasArticleClass
    -0.3793,  // HasMainRole
    0.0,  // HasNavClass
    -0.4702,  // HasSidebarClass
    // TOC signature (48-55)
    -0.1875,  // TocLike
    -1.8015,  // ListLinkInteraction
    0.2742,  // LogNestedListCount
    -0.0112,  // ListItemLinkRatio
    -0.2613,  // ShortTextLinkRatio
    0.2485,  // RepetitiveStructure
    0.0646,  // LogUlOlCount
    -0.1282,  // ConsecutiveLinks
    // Position/context (56-63)
    -0.7405,  // RelativePosition
    -3.2915,  // DistanceFromBody
    0.4672,  // HasParentArticle
    -0.3398,  // HasParentMain
    1.5572,  // SiblingContentRatio
    -0.3592,  // ChildDiversity
    -0.2138,  // TextDensity
    0.3478,  // ContentToBoilerplateRatio
    // Readability-style (64-71)
    0.0301,  // LogCommaCount
    -0.5967,  // AvgParagraphLen
    0.8861,  // IsArticleTag
    1.3272,  // IsMainTag
    0.5864,  // IsSemanticContainer
    -0.4306,  // ParagraphTextScore
    0.5411,  // CleanTextRatio
    -0.6717,  // AncestorArticleDepth
    // Anti-over-extraction (72-79)
    -0.0815,  // ContentDensity
    0.0326,  // LogCleanTextLen
    -0.0108,  // AvgParagraphLenStrict
    0.8653,  // LongParagraphRatio
    0.1181,  // TextToMarkupRatio
    0.2141,  // HasMinParagraphs
    0.1256,  // DescendantDiversity
    0.3565  // ContentClusterScore
];

/// Model bias term
const BIAS: f32 = -4.2058;

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
