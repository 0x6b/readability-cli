use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::{Read, stdin},
    process::exit,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use clap::{Parser, ValueEnum};
use html2md::{
    Handle, NodeData, StructuredPrinter, TagHandler, TagHandlerFactory, parse_html_custom,
};
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle as DomHandle, NodeData as DomNodeData, RcDom};
use rdbl_core::{ExtractOptions, ExtractResult, extract};
use reqwest::{Client, Url, header::CONTENT_TYPE};
use serde_json::{to_string, to_string_pretty};
use sha2::{Digest, Sha256};
use tokio::task::JoinSet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum ImageMode {
    Embed,
    #[default]
    Link,
    Omit,
}

#[derive(Parser)]
#[clap(version, about = "Extract readable content from HTML")]
pub struct Args {
    /// URL to fetch and extract content from
    #[clap(value_parser)]
    pub url: Option<Url>,

    /// Read HTML from stdin instead of URL
    #[clap(long, short)]
    pub stdin: bool,

    /// Output format: markdown, html, text, json
    #[clap(long, short, default_value = "markdown")]
    pub format: String,

    /// Prepend archive metadata as YAML frontmatter (markdown only)
    #[clap(long)]
    pub frontmatter: bool,

    /// Image handling in Markdown: embed, link, or omit
    #[clap(long, value_enum, default_value_t)]
    image_mode: ImageMode,

    /// Increase Markdown heading levels, clamping at h6
    #[clap(long, default_value = "0")]
    pub heading_offset: u8,

    /// Minimum text characters for content (default: 200)
    #[clap(long, default_value = "200")]
    pub min_text: usize,

    /// Enable debug output
    #[clap(long)]
    pub debug: bool,

    /// User agent string for HTTP requests
    #[clap(
        long,
        default_value = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:139.0) Gecko/20100101 Firefox/139.0"
    )]
    pub user_agent: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.format != "markdown" && (args.frontmatter || args.heading_offset > 0) {
        bail!("--frontmatter and --heading-offset can only be used with --format markdown");
    }

    let client = Client::builder().user_agent(&args.user_agent).build()?;

    // Get HTML content and retain the final URL as the base for relative links.
    let (html, document_url) = if args.stdin {
        let mut buffer = String::new();
        stdin().read_to_string(&mut buffer)?;
        (buffer, None)
    } else if let Some(url) = &args.url {
        let (html, final_url) = fetch_url(&client, url).await?;
        (html, Some(final_url))
    } else {
        eprintln!("Error: Either provide a URL or use --stdin to read HTML from stdin");
        exit(1);
    };
    let retrieved_at = format_utc(SystemTime::now())?;

    // Configure extraction options
    let options = ExtractOptions {
        min_text_chars: args.min_text,
        debug: args.debug,
        ..Default::default()
    };

    // Extract content
    let result = extract(&html, &options);
    let embedded_images = if args.image_mode == ImageMode::Embed {
        embed_images(&client, &result.content_html, document_url.as_ref()).await
    } else {
        HashMap::new()
    };

    // Output based on format
    match args.format.as_str() {
        "json" => {
            println!("{}", to_string_pretty(&result)?);
        }
        "html" => {
            if let Some(title) = &result.title {
                println!("<!-- Title: {title} -->");
            }
            println!("{}", result.content_html);
        }
        "text" => {
            if let Some(title) = &result.title {
                println!("{title}\n");
            }
            println!("{}", result.text);
        }
        _ => {
            let source_url = (!args.stdin).then_some(args.url.as_ref()).flatten();
            print!(
                "{}",
                render_markdown(
                    &result,
                    args.frontmatter,
                    args.image_mode,
                    args.heading_offset,
                    source_url,
                    document_url.as_ref(),
                    embedded_images,
                    &retrieved_at,
                )
            );
        }
    }

    Ok(())
}

fn render_markdown(
    result: &ExtractResult,
    frontmatter: bool,
    image_mode: ImageMode,
    heading_offset: u8,
    source_url: Option<&Url>,
    document_url: Option<&Url>,
    embedded_images: HashMap<String, String>,
    retrieved_at: &str,
) -> String {
    let mut markdown = String::new();
    if let Some(title) = &result.title {
        markdown.push_str(&"#".repeat(shifted_heading_level(1, heading_offset)));
        markdown.push(' ');
        markdown.push_str(title);
        markdown.push_str("\n\n");
    }
    markdown.push_str(&markdown_from_html(
        &result.content_html,
        heading_offset,
        image_mode,
        document_url,
        embedded_images,
    ));

    if !frontmatter {
        markdown.push('\n');
        return markdown;
    }

    let hash = format!("{:x}", Sha256::digest(markdown.as_bytes()));
    let mut output = String::from("---\n");
    push_yaml_string(&mut output, "title", result.title.as_deref().unwrap_or(""));
    if let Some(byline) = &result.byline {
        push_yaml_string(&mut output, "byline", byline);
    }
    if let Some(url) = source_url {
        push_yaml_string(&mut output, "source_url", url.as_str());
    }
    push_yaml_string(&mut output, "retrieved_at", retrieved_at);
    push_yaml_string(&mut output, "generator", "rdbl");
    push_yaml_string(&mut output, "generator_version", env!("CARGO_PKG_VERSION"));
    push_yaml_string(&mut output, "content_sha256", &hash);
    output.push_str("---\n");
    output.push_str(&markdown);
    output.push('\n');
    output
}

fn shifted_heading_level(level: u8, offset: u8) -> usize {
    level.saturating_add(offset).min(6).into()
}

fn markdown_from_html(
    html: &str,
    heading_offset: u8,
    image_mode: ImageMode,
    document_url: Option<&Url>,
    embedded_images: HashMap<String, String>,
) -> String {
    let mut handlers: HashMap<String, Box<dyn TagHandlerFactory>> = HashMap::new();
    if heading_offset > 0 {
        for level in 1..=6 {
            handlers.insert(
                format!("h{level}"),
                Box::new(ShiftedHeaderFactory {
                    level: shifted_heading_level(level, heading_offset),
                }),
            );
        }
    }

    let references = Rc::new(RefCell::new(ReferenceRegistry::default()));
    let embedded_images = Rc::new(embedded_images);
    if document_url.is_some() {
        handlers.insert(
            "a".to_string(),
            Box::new(AbsoluteLinkFactory {
                document_url: document_url.cloned(),
            }),
        );
    }
    handlers.insert(
        "img".to_string(),
        Box::new(ReferenceImageFactory {
            document_url: document_url.cloned(),
            image_mode,
            embedded_images: Rc::clone(&embedded_images),
            references: Rc::clone(&references),
        }),
    );

    let mut markdown = parse_html_custom(html, &handlers);
    let definitions = references.borrow().definitions();
    if !definitions.is_empty() {
        markdown.push_str("\n\n");
        markdown.push_str(&definitions);
    }
    markdown
}

#[derive(Default)]
struct ReferenceRegistry {
    entries: Vec<(String, Option<String>)>,
}

impl ReferenceRegistry {
    fn register(&mut self, destination: String, title: Option<String>) -> usize {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.0 == destination && entry.1 == title)
        {
            return index + 1;
        }
        self.entries.push((destination, title));
        self.entries.len()
    }

    fn definitions(&self) -> String {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, (destination, title))| {
                let title = title
                    .as_ref()
                    .map(|title| format!(" {}", to_string(title).unwrap()))
                    .unwrap_or_default();
                let destination = markdown_destination(destination);
                format!("[rdbl-{}]: <{destination}>{title}", index + 1)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

struct AbsoluteLinkFactory {
    document_url: Option<Url>,
}

impl TagHandlerFactory for AbsoluteLinkFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(AbsoluteLink {
            document_url: self.document_url.clone(),
            start_pos: 0,
            destination: None,
        })
    }
}

struct AbsoluteLink {
    document_url: Option<Url>,
    start_pos: usize,
    destination: Option<String>,
}

impl TagHandler for AbsoluteLink {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        self.start_pos = printer.data.len();
        let Some(href) = tag_attribute(tag, "href") else {
            return;
        };
        let destination = resolve_url(&href, self.document_url.as_ref());
        if !destination.is_empty() {
            self.destination = Some(markdown_destination(&destination));
        }
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        if let Some(destination) = &self.destination {
            printer.insert_str(self.start_pos, "[");
            printer.append_str(&format!("](<{destination}>)"));
        }
    }
}

fn markdown_destination(destination: &str) -> String {
    destination
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace(' ', "%20")
        .replace(['\r', '\n'], "")
}

struct ReferenceImageFactory {
    document_url: Option<Url>,
    image_mode: ImageMode,
    embedded_images: Rc<HashMap<String, String>>,
    references: Rc<RefCell<ReferenceRegistry>>,
}

impl TagHandlerFactory for ReferenceImageFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(ReferenceImage {
            document_url: self.document_url.clone(),
            image_mode: self.image_mode,
            embedded_images: Rc::clone(&self.embedded_images),
            references: Rc::clone(&self.references),
        })
    }
}

struct ReferenceImage {
    document_url: Option<Url>,
    image_mode: ImageMode,
    embedded_images: Rc<HashMap<String, String>>,
    references: Rc<RefCell<ReferenceRegistry>>,
}

impl TagHandler for ReferenceImage {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        let alt = tag_attribute(tag, "alt")
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        if self.image_mode == ImageMode::Omit {
            printer.append_str(&format!("[Image omitted: {alt}]"));
            return;
        }
        let Some(src) = tag_attribute(tag, "src") else {
            return;
        };
        let absolute_url = resolve_url(&src, self.document_url.as_ref());
        let destination = if self.image_mode == ImageMode::Embed {
            self.embedded_images
                .get(&absolute_url)
                .cloned()
                .unwrap_or(absolute_url)
        } else {
            absolute_url
        };
        if destination.is_empty() {
            return;
        }
        let title = tag_attribute(tag, "title");
        let reference = self.references.borrow_mut().register(destination, title);
        printer.append_str(&format!("![{alt}][rdbl-{reference}]"));
    }

    fn after_handle(&mut self, _printer: &mut StructuredPrinter) {}
}

fn tag_attribute(tag: &Handle, name: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &tag.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|attr| attr.name.local.as_ref() == name)
        .map(|attr| attr.value.to_string())
}

fn resolve_url(value: &str, document_url: Option<&Url>) -> String {
    document_url
        .and_then(|base| base.join(value).ok())
        .map_or_else(|| value.to_string(), |url| url.to_string())
}

struct ShiftedHeaderFactory {
    level: usize,
}

impl TagHandlerFactory for ShiftedHeaderFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(ShiftedHeader { level: self.level })
    }
}

struct ShiftedHeader {
    level: usize,
}

impl TagHandler for ShiftedHeader {
    fn handle(&mut self, _tag: &Handle, printer: &mut StructuredPrinter) {
        printer.insert_newline();
        printer.insert_newline();
        printer.append_str(&"#".repeat(self.level));
        printer.append_str(" ");
    }

    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        printer.insert_newline();
        printer.insert_newline();
    }
}

fn push_yaml_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(": ");
    // A JSON string is also a valid YAML 1.2 double-quoted scalar.
    output.push_str(&to_string(value).expect("serializing a string cannot fail"));
    output.push('\n');
}

fn format_utc(time: SystemTime) -> Result<String> {
    let seconds = time.duration_since(UNIX_EPOCH)?.as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;

    // Convert days since Unix epoch to a proleptic Gregorian calendar date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);

    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

async fn embed_images(
    client: &Client,
    html: &str,
    document_url: Option<&Url>,
) -> HashMap<String, String> {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .expect("reading HTML from memory cannot fail");
    let mut sources = HashSet::new();
    collect_image_sources(&dom.document, document_url, &mut sources);

    let mut embedded = HashMap::new();
    let sources = sources.into_iter().collect::<Vec<_>>();
    for batch in sources.chunks(8) {
        let mut requests = JoinSet::new();
        for url in batch {
            let client = client.clone();
            let url = url.clone();
            requests.spawn(async move { (url.clone(), fetch_image_data_uri(&client, &url).await) });
        }
        while let Some(result) = requests.join_next().await {
            if let Ok((url, Some(data_uri))) = result {
                embedded.insert(url, data_uri);
            }
        }
    }
    embedded
}

fn collect_image_sources(
    node: &DomHandle,
    document_url: Option<&Url>,
    sources: &mut HashSet<String>,
) {
    if let DomNodeData::Element { name, attrs, .. } = &node.data
        && name.local.as_ref() == "img"
        && let Some(src) = attrs
            .borrow()
            .iter()
            .find(|attr| attr.name.local.as_ref() == "src")
    {
        let url = resolve_url(src.value.as_ref(), document_url);
        if matches!(Url::parse(&url), Ok(url) if matches!(url.scheme(), "http" | "https")) {
            sources.insert(url);
        }
    }
    for child in node.children.borrow().iter() {
        collect_image_sources(child, document_url, sources);
    }
}

async fn fetch_image_data_uri(client: &Client, url: &str) -> Option<String> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let media_type = response
        .headers()
        .get(CONTENT_TYPE)?
        .to_str()
        .ok()?
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase();
    if !media_type.starts_with("image/") {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    Some(format!("data:{media_type};base64,{}", BASE64.encode(bytes)))
}

async fn fetch_url(client: &Client, url: &Url) -> Result<(String, Url)> {
    let response = client.get(url.as_str()).send().await?;

    if !response.status().is_success() {
        bail!("HTTP error: {}", response.status());
    }

    let final_url = response.url().clone();
    let html = response.text().await?;
    Ok((html, final_url))
}

#[cfg(test)]
mod tests {
    #[cfg(test)]
    use std::time::Duration;

    use super::*;

    fn result(title: Option<&str>, byline: Option<&str>) -> ExtractResult {
        ExtractResult {
            title: title.map(str::to_string),
            byline: byline.map(str::to_string),
            content_html: "<p>Archive body: 日本語</p>".to_string(),
            text: "Archive body: 日本語".to_string(),
            debug: None,
        }
    }

    #[test]
    fn default_markdown_output_is_unchanged() {
        assert_eq!(
            render_markdown(
                &result(Some("Article"), None),
                false,
                ImageMode::Link,
                0,
                None,
                None,
                HashMap::new(),
                "ignored",
            ),
            "# Article\n\nArchive body: 日本語\n"
        );
    }

    #[test]
    fn frontmatter_quotes_special_strings_and_hashes_exact_body() {
        let result = result(Some("題名: \"引用\"\n次の行"), Some("著者: Jane's \"J\""));
        let url = Url::parse("https://example.com/path?q=a:b&name=%E6%97%A5%E6%9C%AC").unwrap();
        let body = "### 題名: \"引用\"\n次の行\n\nArchive body: 日本語";
        let hash = format!("{:x}", Sha256::digest(body.as_bytes()));

        let output = render_markdown(
            &result,
            true,
            ImageMode::Link,
            2,
            Some(&url),
            Some(&url),
            HashMap::new(),
            "2026-08-31T12:34:56Z",
        );

        assert!(output.contains("title: \"題名: \\\"引用\\\"\\n次の行\"\n"));
        assert!(output.contains("byline: \"著者: Jane's \\\"J\\\"\"\n"));
        assert!(
            output.contains(
                "source_url: \"https://example.com/path?q=a:b&name=%E6%97%A5%E6%9C%AC\"\n"
            )
        );
        assert!(output.contains(&format!("content_sha256: \"{hash}\"\n")));
        assert!(output.ends_with(&format!("---\n{body}\n")));
    }

    #[test]
    fn frontmatter_omits_optional_byline_and_source_url() {
        let output = render_markdown(
            &result(Some("No author"), None),
            true,
            ImageMode::Link,
            0,
            None,
            None,
            HashMap::new(),
            "2026-08-31T00:00:00Z",
        );
        assert!(!output.contains("byline:"));
        assert!(!output.contains("source_url:"));
    }

    #[test]
    fn heading_offset_uses_html_structure_and_clamps_at_h6() {
        let mut result = result(Some("題名"), None);
        result.content_html =
            "<h1>見出し一</h1><h5>Heading five</h5><h6>見出し六</h6><pre><code># unchanged\n## code</code></pre>"
                .to_string();

        let output = render_markdown(
            &result,
            false,
            ImageMode::Link,
            2,
            None,
            None,
            HashMap::new(),
            "ignored",
        );

        assert!(output.starts_with("### 題名\n\n### 見出し一"));
        assert!(output.contains("###### Heading five"));
        assert!(output.contains("###### 見出し六"));
        assert!(output.contains("```\n# unchanged\n## code\n```"));
    }

    #[test]
    fn portable_markdown_keeps_links_inline_and_collects_images_at_end() {
        let base = Url::parse("https://example.com/articles/page.html").unwrap();
        let data_uri = "data:image/png;base64,AQID";
        let mut images = HashMap::new();
        images.insert(
            "https://example.com/media/photo.png".to_string(),
            data_uri.to_string(),
        );
        let html = concat!(
            "<p><a href=\"../about?q=1\">About</a> and ",
            "<a href=\"../about?q=1\">again</a>.</p>",
            "<img src=\"../media/photo.png\" alt=\"Photo\" title=\"Saved image\">"
        );

        let output = markdown_from_html(html, 0, ImageMode::Embed, Some(&base), images);

        assert_eq!(
            output,
            concat!(
                "[About](<https://example.com/about?q=1>) and ",
                "[again](<https://example.com/about?q=1>).\n\n",
                "![Photo][rdbl-1]\n\n",
                "[rdbl-1]: <data:image/png;base64,AQID> \"Saved image\""
            )
        );
    }

    #[test]
    fn image_modes_are_independent_from_frontmatter() {
        let base = Url::parse("https://example.com/articles/page.html").unwrap();
        let html = "<p>Body</p><img src=\"../photo.png\" alt=\"Photo\">";
        let mut images = HashMap::new();
        images.insert(
            "https://example.com/photo.png".to_string(),
            "data:image/png;base64,AQID".to_string(),
        );

        let linked = markdown_from_html(html, 0, ImageMode::Link, Some(&base), images.clone());
        let embedded = markdown_from_html(html, 0, ImageMode::Embed, Some(&base), images.clone());
        let omitted = markdown_from_html(html, 0, ImageMode::Omit, Some(&base), images);

        assert!(linked.contains("[rdbl-1]: <https://example.com/photo.png>"));
        assert!(embedded.contains("[rdbl-1]: <data:image/png;base64,AQID>"));
        assert_eq!(omitted, "Body\n\n[Image omitted: Photo]");
        assert!(!omitted.contains("photo.png"));
    }

    #[test]
    fn formats_utc_timestamp() {
        let time = UNIX_EPOCH + Duration::from_secs(1_788_179_696);
        assert_eq!(format_utc(time).unwrap(), "2026-08-31T12:34:56Z");
    }
}
