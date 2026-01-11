//! WASM wrapper for readable_core
//!
//! Provides a JavaScript-friendly API for content extraction.

use readable_core::{ExtractOptions, ExtractResult};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Extract readable content from HTML
///
/// # Arguments
/// * `html` - The HTML string to extract content from
/// * `opts` - Optional extraction options as a JavaScript object
///
/// # Returns
/// A JavaScript object with title, byline, content_html, text, and optionally debug info
#[wasm_bindgen]
pub fn extract(html: &str, opts: JsValue) -> Result<JsValue, JsValue> {
    // Parse options
    let options: ExtractOptions = if opts.is_undefined() || opts.is_null() {
        ExtractOptions::default()
    } else {
        serde_wasm_bindgen::from_value(opts).map_err(|e| JsValue::from_str(&e.to_string()))?
    };

    // Run extraction
    let result = readable_core::extract(html, &options);

    // Convert to JS
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic() {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Page</title></head>
        <body>
            <article>
                <h1>Test Article</h1>
                <p>This is some test content for the article.</p>
                <p>More content here to make it substantial enough.</p>
            </article>
        </body>
        </html>
        "#;

        let options = ExtractOptions::default();
        let result = readable_core::extract(html, &options);

        assert!(result.title.is_some());
        assert!(!result.content_html.is_empty() || !result.text.is_empty());
    }
}
