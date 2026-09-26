use std::collections::HashMap;

use rdbl::{ExtractOptions, ImageMode, RenderOptions, Url, extract_and_render};

#[test]
fn fetched_html_and_final_url_produce_complete_archive() {
    let html = r#"<html><head><title>Public API</title><meta name="author" content="Ada"></head>
        <body><article><h1>Public API</h1>
        <p>This article contains enough meaningful text for the extractor and verifies that an
        external consumer can render the same archive without allowing this crate to perform I/O.</p>
        <p><a href="../details">Details</a></p></article></body></html>"#;
    let source = Url::parse("https://example.com/redirect").unwrap();
    let final_url = Url::parse("https://www.example.com/articles/page").unwrap();

    let archive = extract_and_render(
        html,
        &ExtractOptions {
            min_text_chars: 20,
            ..Default::default()
        },
        RenderOptions {
            frontmatter: true,
            image_mode: ImageMode::Omit,
            heading_offset: 1,
            source_url: Some(&source),
            document_url: Some(&final_url),
            embedded_images: HashMap::new(),
            retrieved_at: "2026-09-26T00:00:00Z",
        },
    );

    assert_eq!(archive.extracted.title.as_deref(), Some("Public API"));
    assert_eq!(archive.extracted.byline.as_deref(), Some("Ada"));
    assert!(
        archive
            .rendered
            .content
            .contains("source_url: \"https://example.com/redirect\"")
    );
    assert!(
        archive
            .rendered
            .content
            .contains("[Details](<https://www.example.com/details>)")
    );
    assert!(archive.rendered.body.starts_with("## Public API"));
    assert_eq!(archive.rendered.content_sha256.len(), 64);
}
