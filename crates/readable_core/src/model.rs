//! Logistic regression model for candidate scoring
//!
//! Uses embedded weights trained via teacher-student learning from Readability.js.

use crate::features::{FeatureVector, NUM_FEATURES};

/// Logistic regression model with embedded weights
pub struct LogisticModel {
    /// Feature weights
    weights: [f32; NUM_FEATURES],
    /// Bias term
    bias: f32,
}

impl LogisticModel {
    /// Create model with the embedded weights
    pub fn new() -> Self {
        Self { weights: WEIGHTS, bias: BIAS }
    }

    /// Compute the logit (raw score before sigmoid)
    pub fn score_logit(&self, features: &FeatureVector) -> f32 {
        let mut logit = self.bias;
        for (w, x) in self.weights.iter().zip(features.values.iter()) {
            logit += w * x;
        }
        logit
    }

    /// Compute the probability (sigmoid of logit)
    #[allow(dead_code)]
    pub fn score_prob(&self, features: &FeatureVector) -> f32 {
        sigmoid(self.score_logit(features))
    }
}

impl Default for LogisticModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Sigmoid function
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Score a feature vector using the global model
pub fn score(features: &FeatureVector) -> f32 {
    MODEL.score_logit(features)
}

// Global model instance
lazy_static::lazy_static! {
    static ref MODEL: LogisticModel = LogisticModel::new();
}

// =============================================================================
// EMBEDDED WEIGHTS
// =============================================================================
// These weights are trained using tools/train_logreg.py and exported via
// tools/export_weights.py. The feature order must match features.rs exactly.
//
// Feature indices (for reference):
//  0: LogTextLenChars      16: LogATagCount         32: DepthNorm            48: TocLike
//  1: LogTextLenBytes      17: LogLiCount           33: LogSubtreeNodeCount  49:
// ListLinkInteraction  2: LogWordLikeCount     18: LogButtonInputForm   34: LogPCount
// 50: LogNestedListCount  3: LogCjkCharCount      19: LinkDensity          35: LogHCount
// 51: ListItemLinkRatio  4: WhitespaceRatio      20: NavTagInSubtree      36: LogImgCount
// 52: ShortTextLinkRatio  5: LineCount            21: LogFormElementCount  37: SemanticMainFlag
// 53: RepetitiveStructure  6: ShortLineRatio       22: LogScriptStyleCount  38: TagPrior
// 54: LogUlOlCount  7: AvgLineLen           23: InlineStyleCount     39: LogTableCount        55:
// ConsecutiveLinks  8: DotQRatio            24: LogPreCount          40: LogPosKwHits         56:
// RelativePosition  9: JpPunctRatio         25: LogCodeCount         41: LogNegKwHits         57:
// DistanceFromBody 10: CommaRatio           26: LogCodeBlockTextLen  42: PosMinusNeg          58:
// HasParentArticle 11: QuoteRatio           27: InlineCodeRatio      43: HasContentClass      59:
// HasParentMain 12: SentenceEndRatio     28: BraceRatio           44: HasArticleClass      60:
// SiblingContentRatio 13: ColonRatio           29: BacktickRatio        45: HasMainRole
// 61: ChildDiversity 14: SemicolonRatio       30: IndentLineRatio      46: HasNavClass          62:
// TextDensity 15: ParenRatio           31: CamelCaseRatio       47: HasSidebarClass      63:
// ContentToBoilerplate

/// Model weights - placeholder values, replace with trained weights
const WEIGHTS: [f32; NUM_FEATURES] = [
    // Text quantity (0-7) - larger text is generally better
    0.8,  // LogTextLenChars
    0.1,  // LogTextLenBytes (redundant, low weight)
    0.5,  // LogWordLikeCount
    0.3,  // LogCjkCharCount
    -0.2, // WhitespaceRatio
    0.2,  // LineCount
    -0.8, // ShortLineRatio (many short lines = TOC/nav)
    0.3,  // AvgLineLen
    // Punctuation (8-15)
    0.4, // DotQRatio (sentences)
    0.4, // JpPunctRatio (Japanese sentences)
    0.2, // CommaRatio
    0.1, // QuoteRatio
    0.3, // SentenceEndRatio
    0.0, // ColonRatio
    0.0, // SemicolonRatio
    0.0, // ParenRatio
    // Link/boilerplate (16-23)
    -0.3, // LogATagCount (many links = nav)
    -0.5, // LogLiCount (many list items = nav/TOC)
    -0.6, // LogButtonInputFormCount
    -1.5, // LinkDensity (high link density = nav)
    -1.0, // NavTagInSubtreeCount
    -0.5, // LogFormElementCount
    -0.3, // LogScriptStyleCount
    -0.1, // InlineStyleCount
    // Code-ness (24-31)
    0.6, // LogPreCount (code blocks are content)
    0.3, // LogCodeCount
    0.5, // LogCodeBlockTextLen
    0.2, // InlineCodeRatio
    0.1, // BraceRatio
    0.1, // BacktickRatio
    0.2, // IndentLineRatio
    0.1, // CamelCaseRatio
    // Structure (32-39)
    -0.3, // DepthNorm (too deep = nested boilerplate)
    0.2,  // LogSubtreeNodeCount
    0.6,  // LogPCount (paragraphs = content)
    0.4,  // LogHCount (headings = content)
    0.2,  // LogImgCount
    1.5,  // SemanticMainFlag (article/main = strong signal)
    0.8,  // TagPrior
    0.1,  // LogTableCount
    // Keywords (40-47)
    0.8,  // LogPosKwHits
    -0.8, // LogNegKwHits
    1.0,  // PosMinusNeg
    0.6,  // HasContentClass
    0.8,  // HasArticleClass
    1.2,  // HasMainRole
    -1.0, // HasNavClass
    -0.8, // HasSidebarClass
    // TOC signature (48-55)
    -1.5, // TocLike
    -1.2, // ListLinkInteraction
    -0.4, // LogNestedListCount
    -0.8, // ListItemLinkRatio
    -0.5, // ShortTextLinkRatio
    -1.0, // RepetitiveStructure
    -0.3, // LogUlOlCount
    -0.4, // ConsecutiveLinks
    // Position/context (56-63)
    -0.2, // RelativePosition (content often in middle)
    -0.1, // DistanceFromBody
    0.5,  // HasParentArticle
    0.6,  // HasParentMain
    0.4,  // SiblingContentRatio
    0.2,  // ChildDiversity
    0.3,  // TextDensity
    0.5,  // ContentToBoilerplateRatio
];

/// Model bias term - placeholder value
const BIAS: f32 = -2.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_logit() {
        let model = LogisticModel::new();
        let features = FeatureVector::default();
        let score = model.score_logit(&features);

        // With zero features, should return bias
        assert!((score - BIAS).abs() < 0.001);
    }

    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(0.0) - 0.5).abs() < 0.001);
        assert!(sigmoid(10.0) > 0.99);
        assert!(sigmoid(-10.0) < 0.01);
    }

    #[test]
    fn test_semantic_features_increase_score() {
        let model = LogisticModel::new();

        let mut features = FeatureVector::default();
        let base_score = model.score_logit(&features);

        // Set semantic main flag
        features.values[37] = 1.0; // SemanticMainFlag
        let semantic_score = model.score_logit(&features);

        assert!(semantic_score > base_score, "Semantic flag should increase score");
    }

    #[test]
    fn test_link_density_decreases_score() {
        let model = LogisticModel::new();

        let mut features = FeatureVector::default();
        let base_score = model.score_logit(&features);

        // Set high link density
        features.values[19] = 0.9; // LinkDensity
        let link_score = model.score_logit(&features);

        assert!(link_score < base_score, "High link density should decrease score");
    }
}
