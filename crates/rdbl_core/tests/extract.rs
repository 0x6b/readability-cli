//! Public extraction API integration tests.

use rdbl_core::{ExtractOptions, extract};

#[test]
fn test_extract_article() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Test Article - My Blog</title>
        <meta property="og:title" content="Test Article">
    </head>
    <body>
        <nav>
            <a href="/">Home</a>
            <a href="/about">About</a>
        </nav>
        <article class="main-content">
            <h1>Test Article</h1>
            <p>This is the first paragraph of the article. It contains some meaningful content
            that should be extracted by the readability algorithm.</p>
            <p>Here is another paragraph with more content. The algorithm should recognize
            that this is the main body of the article and extract it properly.</p>
            <p>A third paragraph to make the content substantial enough for extraction.
            We need enough text to pass the minimum threshold.</p>
        </article>
        <aside class="sidebar">
            <h3>Related Posts</h3>
            <ul>
                <li><a href="/post1">Post 1</a></li>
                <li><a href="/post2">Post 2</a></li>
            </ul>
        </aside>
        <footer>
            <p>Copyright 2024</p>
        </footer>
    </body>
    </html>
    "#;

    let options = ExtractOptions { min_text_chars: 100, ..Default::default() };

    let result = extract(html, &options);

    // Should extract title
    assert!(result.title.is_some());
    assert_eq!(result.title.as_deref(), Some("Test Article"));

    // Should have content
    assert!(!result.text.is_empty());

    // Content should include article text
    assert!(result.text.contains("first paragraph"));
    assert!(result.text.contains("meaningful content"));

    // Content should NOT include navigation
    assert!(!result.text.contains("Home"));
    assert!(!result.text.contains("About"));

    // Content should NOT include sidebar
    assert!(!result.text.contains("Related Posts"));
}

#[test]
fn test_extract_main_does_not_expand_to_external_sibling() {
    let html = r#"
    <html><body>
        <main class="article-content">
            <h1>Selected article</h1>
            <p>The first article paragraph contains enough meaningful text to identify this
            semantic main element as the primary content of the page.</p>
            <p>The second article paragraph continues the report with relevant details and
            context that belong in the extracted result.</p>
            <p>The final article paragraph completes the story for readers.</p>
        </main>
        <div class="below-content">
            <p>External sibling teaser one contains substantial descriptive copy that could
            otherwise satisfy sibling expansion thresholds.</p>
            <p>External sibling teaser two is not part of the selected semantic main element
            and must therefore remain outside the extraction.</p>
        </div>
    </body></html>
    "#;
    let options = ExtractOptions { min_text_chars: 100, ..Default::default() };

    let result = extract(html, &options);

    assert!(result.text.contains("first article paragraph"));
    assert!(!result.text.contains("External sibling teaser"));
}

#[test]
fn test_extract_code_heavy_page() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head><title>Code Example</title></head>
    <body>
        <article>
            <h1>Rust Code Example</h1>
            <p>Here is a simple Rust program:</p>
            <pre><code>fn main() {
    println!("Hello, world!");

    for i in 0..10 {
        println!("Number: {}", i);
    }
}</code></pre>
            <p>This code demonstrates basic Rust syntax including functions and loops.</p>
        </article>
    </body>
    </html>
    "#;

    let options = ExtractOptions::default();
    let result = extract(html, &options);

    // Should preserve code content
    assert!(result.text.contains("fn main()") || result.content_html.contains("fn main()"));
}

#[test]
fn test_extract_japanese_content() {
    let html = r#"
    <!DOCTYPE html>
    <html lang="ja">
    <head>
        <title>日本語の記事</title>
        <meta property="og:title" content="日本語テスト">
    </head>
    <body>
        <nav>ホーム | ブログ | お問い合わせ</nav>
        <article class="本文">
            <h1>日本語の記事</h1>
            <p>これは日本語のテスト記事です。文章が正しく抽出されることを確認します。
            日本語の句読点も正しく処理される必要があります。</p>
            <p>二つ目の段落です。より多くのコンテンツを追加して、
            抽出アルゴリズムがテストできるようにします。日本語の文章は
            英語とは異なる構造を持っています。</p>
        </article>
        <footer>著作権 2024</footer>
    </body>
    </html>
    "#;

    let options = ExtractOptions { min_text_chars: 50, ..Default::default() };

    let result = extract(html, &options);

    // Should extract Japanese title
    assert!(result.title.is_some());

    // Should extract Japanese content
    assert!(result.text.contains("日本語"));
    assert!(result.text.contains("テスト記事"));
}

#[test]
fn test_debug_mode() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head><title>Debug Test</title></head>
    <body>
        <article>
            <p>Some content for debug testing. This needs to be long enough
            to be considered a valid candidate for extraction.</p>
        </article>
    </body>
    </html>
    "#;

    let options = ExtractOptions {
        debug: true,
        min_text_chars: 50,
        ..Default::default()
    };

    let result = extract(html, &options);

    // Should include debug info
    assert!(result.debug.is_some());
    let debug = result.debug.unwrap();
    assert!(!debug.candidates.is_empty());
}

#[test]
fn test_extract_oembed_video() {
    let html = r#"<oembed>
        <title>Video title</title>
        <type>video</type>
        <html>&lt;iframe src=&quot;https://www.youtube.com/embed/video-id&quot;
            title=&quot;Video title&quot;&gt;&lt;/iframe&gt;</html>
    </oembed>"#;

    let result = extract(html, &ExtractOptions::default());

    assert_eq!(result.title.as_deref(), Some("Video title"));
    assert!(result.content_html.contains("<iframe"));
    assert!(result.content_html.contains("https://www.youtube.com/embed/video-id"));
}
