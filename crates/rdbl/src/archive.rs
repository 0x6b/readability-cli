use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    error::Error,
    fmt::{self, Display, Formatter},
    future::Future,
    rc::Rc,
    str::FromStr,
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use html2md::{
    Handle, NodeData, StructuredPrinter, TagHandler, TagHandlerFactory, parse_html_custom,
};
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use sha2::{Digest, Sha256};
use url::Url;

use crate::{ExtractOptions, ExtractResult, extract};

pub const GENERATOR: &str = "rdbl";
pub const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageMode {
    Embed,
    #[default]
    Link,
    Omit,
}

impl Display for ImageMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Embed => "embed",
            Self::Link => "link",
            Self::Omit => "omit",
        })
    }
}

impl FromStr for ImageMode {
    type Err = ParseImageModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "embed" => Ok(Self::Embed),
            "link" => Ok(Self::Link),
            "omit" => Ok(Self::Omit),
            _ => Err(ParseImageModeError(value.into())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseImageModeError(String);

impl Display for ParseImageModeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid image mode {:?}; expected embed, link, or omit",
            self.0
        )
    }
}

impl Error for ParseImageModeError {}

#[derive(Clone, Debug)]
pub struct RenderOptions<'a> {
    pub frontmatter: bool,
    pub image_mode: ImageMode,
    pub heading_offset: u8,
    /// URL recorded in frontmatter and the default relative-URL resolution root.
    /// May be omitted for URL-less input.
    pub source_url: Option<&'a Url>,
    /// Override the relative-URL resolution root. Defaults to `source_url`.
    pub base_url: Option<&'a Url>,
    pub retrieved_at: &'a str,
    /// Maximum bytes in the exact rendered content, including frontmatter.
    pub max_output_bytes: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchedImage {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

impl FetchedImage {
    fn data_uri(&self) -> Option<String> {
        let valid_media_type = self.media_type.starts_with("image/")
            && self.media_type.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'/' | b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            });
        valid_media_type.then(|| image_data_uri(&self.media_type, &self.bytes))
    }
}

pub trait ImageFetcher {
    /// Fetch an image only if its resulting data URI fits in `max_data_uri_bytes`.
    fn fetch_image(
        &self,
        url: &Url,
        max_data_uri_bytes: usize,
    ) -> impl Future<Output = Option<FetchedImage>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchedHtml {
    pub html: String,
    /// Final URL after redirects, used to resolve relative links and images.
    pub final_url: Url,
}

pub trait HtmlFetcher {
    type Error: Error + Send + Sync + 'static;

    fn fetch_html(
        &self,
        url: &Url,
    ) -> impl Future<Output = Result<FetchedHtml, Self::Error>> + Send;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveError {
    OutputTooLarge { limit: usize, actual: usize },
}

impl Display for ArchiveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputTooLarge { limit, actual } => {
                write!(
                    formatter,
                    "rendered content is {actual} bytes; limit is {limit} bytes"
                )
            }
        }
    }
}

impl Error for ArchiveError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedDocument {
    /// Exact output, including frontmatter and the CLI-compatible final newline.
    pub content: String,
    /// Markdown bytes covered by `content_sha256`.
    pub body: String,
    pub content_sha256: String,
}

impl Display for RenderedDocument {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.content)
    }
}

#[derive(Clone, Debug)]
pub struct ArchiveDocument {
    pub extracted: ExtractResult,
    pub rendered: RenderedDocument,
}

/// Extract readable content from fetched HTML and render the exact archive output.
///
/// This function performs no I/O. `source_url` is recorded in archive metadata,
/// while `base_url` is the resolution root for relative links. Network-facing
/// consumers retain complete control of DNS, redirects, response limits,
/// timeouts, and image fetching.
pub fn extract_and_render(
    html: &str,
    extract_options: &ExtractOptions,
    render_options: RenderOptions<'_>,
) -> Result<ArchiveDocument, ArchiveError> {
    let extracted = extract(html, extract_options);
    let rendered = render_markdown(&extracted, render_options)?;
    Ok(ArchiveDocument {
        extracted,
        rendered,
    })
}

pub async fn extract_and_render_with_images<F: ImageFetcher>(
    html: &str,
    extract_options: &ExtractOptions,
    render_options: RenderOptions<'_>,
    fetcher: &F,
) -> Result<ArchiveDocument, ArchiveError> {
    let extracted = extract(html, extract_options);
    let rendered = render_markdown_with_images(&extracted, render_options, fetcher).await?;
    Ok(ArchiveDocument {
        extracted,
        rendered,
    })
}

pub fn render_markdown(
    result: &ExtractResult,
    options: RenderOptions<'_>,
) -> Result<RenderedDocument, ArchiveError> {
    let prepared = prepare_markdown(result, &options);
    finish_markdown(result, &options, prepared, &HashMap::new())
}

pub async fn render_markdown_with_images<F: ImageFetcher>(
    result: &ExtractResult,
    options: RenderOptions<'_>,
    fetcher: &F,
) -> Result<RenderedDocument, ArchiveError> {
    let prepared = prepare_markdown(result, &options);
    let baseline = finish_markdown(result, &options, prepared.clone(), &HashMap::new())?;
    if options.image_mode != ImageMode::Embed {
        return Ok(baseline);
    }

    let mut current_size = baseline.content.len();
    let mut embedded = HashMap::new();
    let mut seen = HashSet::new();
    for reference in &prepared.references {
        if !seen.insert(reference.destination.clone()) {
            continue;
        }
        let Ok(url) = Url::parse(&reference.destination) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let reference_count = prepared
            .references
            .iter()
            .filter(|candidate| candidate.destination == reference.destination)
            .count();
        let fallback_len = markdown_destination(&reference.destination).len();
        let remaining = options
            .max_output_bytes
            .map_or(usize::MAX, |limit| limit.saturating_sub(current_size));
        let growth_per_reference = remaining / reference_count;
        let max_data_uri_bytes = fallback_len.saturating_add(growth_per_reference);
        let Some(image) = fetcher.fetch_image(&url, max_data_uri_bytes).await else {
            continue;
        };
        let Some(data_uri) = image.data_uri() else {
            continue;
        };
        if data_uri.len() > max_data_uri_bytes {
            continue;
        }
        let growth = data_uri
            .len()
            .saturating_sub(fallback_len)
            .saturating_mul(reference_count);
        let Some(candidate_size) = current_size.checked_add(growth) else {
            continue;
        };
        if options
            .max_output_bytes
            .is_some_and(|limit| candidate_size > limit)
        {
            continue;
        }
        current_size = candidate_size;
        embedded.insert(reference.destination.clone(), data_uri);
    }

    finish_markdown(result, &options, prepared, &embedded)
}

#[derive(Clone)]
struct PreparedMarkdown {
    body: String,
    references: Vec<Reference>,
}

#[derive(Clone)]
struct Reference {
    destination: String,
    title: Option<String>,
}

fn prepare_markdown(result: &ExtractResult, options: &RenderOptions<'_>) -> PreparedMarkdown {
    let base_url = options.base_url.or(options.source_url);
    let mut body = String::new();
    if let Some(title) = &result.title {
        body.push_str(&"#".repeat(shifted_heading_level(1, options.heading_offset)));
        body.push(' ');
        body.push_str(title);
        body.push_str("\n\n");
    }
    let (markdown, references) = markdown_from_html(
        &result.content_html,
        options.heading_offset,
        options.image_mode,
        base_url,
    );
    body.push_str(&markdown);
    PreparedMarkdown { body, references }
}

fn finish_markdown(
    result: &ExtractResult,
    options: &RenderOptions<'_>,
    mut prepared: PreparedMarkdown,
    embedded: &HashMap<String, String>,
) -> Result<RenderedDocument, ArchiveError> {
    if !prepared.references.is_empty() {
        prepared.body.push_str("\n\n");
        prepared
            .body
            .push_str(&reference_definitions(&prepared.references, embedded));
    }
    let body = prepared.body;
    let content_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));

    let mut content = String::new();
    if options.frontmatter {
        content.push_str("---\n");
        push_yaml_string(&mut content, "title", result.title.as_deref().unwrap_or(""));
        if let Some(byline) = &result.byline {
            push_yaml_string(&mut content, "byline", byline);
        }
        if let Some(url) = options.source_url {
            push_yaml_string(&mut content, "source_url", url.as_str());
        }
        push_yaml_string(&mut content, "retrieved_at", options.retrieved_at);
        push_yaml_string(&mut content, "generator", GENERATOR);
        push_yaml_string(&mut content, "generator_version", GENERATOR_VERSION);
        push_yaml_string(&mut content, "content_sha256", &content_sha256);
        content.push_str("---\n");
    }
    content.push_str(&body);
    content.push('\n');

    if let Some(limit) = options.max_output_bytes
        && content.len() > limit
    {
        return Err(ArchiveError::OutputTooLarge {
            limit,
            actual: content.len(),
        });
    }

    Ok(RenderedDocument {
        content,
        body,
        content_sha256,
    })
}

pub fn format_utc(time: SystemTime) -> Result<String, SystemTimeError> {
    let seconds = time.duration_since(UNIX_EPOCH)?.as_secs();
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
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

fn image_data_uri(media_type: &str, bytes: &[u8]) -> String {
    format!("data:{media_type};base64,{}", BASE64.encode(bytes))
}

fn markdown_from_html(
    html: &str,
    heading_offset: u8,
    image_mode: ImageMode,
    base_url: Option<&Url>,
) -> (String, Vec<Reference>) {
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
    if base_url.is_some() {
        handlers.insert(
            "a".into(),
            Box::new(AbsoluteLinkFactory {
                base_url: base_url.cloned(),
            }),
        );
    }
    let references = Rc::new(RefCell::new(ReferenceRegistry::default()));
    handlers.insert(
        "img".into(),
        Box::new(ReferenceImageFactory {
            base_url: base_url.cloned(),
            image_mode,
            references: Rc::clone(&references),
        }),
    );
    let markdown = parse_html_custom(html, &handlers);
    let references = references.borrow().entries.clone();
    (markdown, references)
}

fn shifted_heading_level(level: u8, offset: u8) -> usize {
    level.saturating_add(offset).min(6).into()
}

#[derive(Default)]
struct ReferenceRegistry {
    entries: Vec<Reference>,
}

impl ReferenceRegistry {
    fn register(&mut self, destination: String, title: Option<String>) -> usize {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.destination == destination && entry.title == title)
        {
            return index + 1;
        }
        self.entries.push(Reference { destination, title });
        self.entries.len()
    }
}

fn reference_definitions(references: &[Reference], embedded: &HashMap<String, String>) -> String {
    references
        .iter()
        .enumerate()
        .map(|(index, reference)| {
            let title = reference
                .title
                .as_ref()
                .map(|title| format!(" {}", to_string(title).unwrap()))
                .unwrap_or_default();
            let destination = embedded
                .get(&reference.destination)
                .unwrap_or(&reference.destination);
            format!(
                "[rdbl-{}]: <{}>{title}",
                index + 1,
                markdown_destination(destination)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct AbsoluteLinkFactory {
    base_url: Option<Url>,
}
impl TagHandlerFactory for AbsoluteLinkFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(AbsoluteLink {
            base_url: self.base_url.clone(),
            start_pos: 0,
            destination: None,
        })
    }
}
struct AbsoluteLink {
    base_url: Option<Url>,
    start_pos: usize,
    destination: Option<String>,
}
impl TagHandler for AbsoluteLink {
    fn handle(&mut self, tag: &Handle, printer: &mut StructuredPrinter) {
        self.start_pos = printer.data.len();
        if let Some(href) = tag_attribute(tag, "href") {
            let destination = resolve_url(&href, self.base_url.as_ref());
            if !destination.is_empty() {
                self.destination = Some(markdown_destination(&destination));
            }
        }
    }
    fn after_handle(&mut self, printer: &mut StructuredPrinter) {
        if let Some(destination) = &self.destination {
            printer.insert_str(self.start_pos, "[");
            printer.append_str(&format!("](<{destination}>)"));
        }
    }
}

struct ReferenceImageFactory {
    base_url: Option<Url>,
    image_mode: ImageMode,
    references: Rc<RefCell<ReferenceRegistry>>,
}
impl TagHandlerFactory for ReferenceImageFactory {
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(ReferenceImage {
            base_url: self.base_url.clone(),
            image_mode: self.image_mode,
            references: Rc::clone(&self.references),
        })
    }
}
struct ReferenceImage {
    base_url: Option<Url>,
    image_mode: ImageMode,
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
        let absolute_url = resolve_url(&src, self.base_url.as_ref());
        if absolute_url.is_empty() {
            return;
        }
        let reference = self
            .references
            .borrow_mut()
            .register(absolute_url, tag_attribute(tag, "title"));
        printer.append_str(&format!("![{alt}][rdbl-{reference}]"));
    }
    fn after_handle(&mut self, _printer: &mut StructuredPrinter) {}
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
fn resolve_url(value: &str, base_url: Option<&Url>) -> String {
    base_url
        .and_then(|base| base.join(value).ok())
        .map_or_else(|| value.to_string(), |url| url.to_string())
}
fn markdown_destination(destination: &str) -> String {
    destination
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace(' ', "%20")
        .replace(['\r', '\n'], "")
}
fn push_yaml_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(": ");
    output.push_str(&to_string(value).expect("serializing a string cannot fail"));
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result() -> ExtractResult {
        ExtractResult {
            title: Some("Article".into()),
            byline: Some("Author".into()),
            content_html: "<h1>Section</h1><p>Body</p><img src=\"/photo.png\" alt=\"Photo\">"
                .into(),
            text: "Body".into(),
            debug: None,
        }
    }

    #[test]
    fn renders_cli_compatible_archive() {
        let url = Url::parse("https://example.com/a").unwrap();
        let rendered = render_markdown(
            &result(),
            RenderOptions {
                frontmatter: true,
                image_mode: ImageMode::Link,
                heading_offset: 1,
                source_url: Some(&url),
                base_url: None,
                retrieved_at: "2026-01-02T03:04:05Z",
                max_output_bytes: None,
            },
        )
        .unwrap();
        assert!(rendered.content.starts_with("---\ntitle: \"Article\""));
        assert!(rendered.body.starts_with("## Article\n\n## Section"));
        assert!(
            rendered
                .content
                .contains("[rdbl-1]: <https://example.com/photo.png>")
        );
    }

    #[test]
    fn default_output_remains_byte_compatible() {
        let mut result = result();
        result.content_html = "<p>Archive body: 日本語</p>".into();
        let rendered = render_markdown(
            &result,
            RenderOptions {
                frontmatter: false,
                image_mode: ImageMode::Link,
                heading_offset: 0,
                source_url: None,
                base_url: None,
                retrieved_at: "ignored",
                max_output_bytes: None,
            },
        )
        .unwrap();

        assert_eq!(rendered.content, "# Article\n\nArchive body: 日本語\n");
        assert_eq!(rendered.body, "# Article\n\nArchive body: 日本語");
        assert_eq!(
            rendered.content_sha256,
            "94af606d01b6e332331c1a9fbb02346eadadee45812a1f356b4f23f137e8257a"
        );

        let error = render_markdown(
            &result,
            RenderOptions {
                frontmatter: false,
                image_mode: ImageMode::Link,
                heading_offset: 0,
                source_url: None,
                base_url: None,
                retrieved_at: "ignored",
                max_output_bytes: Some(rendered.content.len() - 1),
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            ArchiveError::OutputTooLarge {
                limit: rendered.content.len() - 1,
                actual: rendered.content.len(),
            }
        );
    }

    struct StaticFetcher {
        image: FetchedImage,
    }

    impl ImageFetcher for StaticFetcher {
        async fn fetch_image(&self, _url: &Url, max_data_uri_bytes: usize) -> Option<FetchedImage> {
            (self.image.data_uri()?.len() <= max_data_uri_bytes).then(|| self.image.clone())
        }
    }

    #[tokio::test]
    async fn fetches_embedded_images_and_falls_back_at_output_limit() {
        let base = Url::parse("https://example.com/articles/page").unwrap();
        let fetcher = StaticFetcher {
            image: FetchedImage {
                media_type: "image/png".into(),
                bytes: vec![1; 30],
            },
        };
        let embedded = render_markdown_with_images(
            &result(),
            RenderOptions {
                frontmatter: false,
                image_mode: ImageMode::Embed,
                heading_offset: 0,
                source_url: Some(&base),
                base_url: None,
                retrieved_at: "ignored",
                max_output_bytes: None,
            },
            &fetcher,
        )
        .await
        .unwrap();
        let fallback_limit = embedded.content.len() - 1;
        let fallback = render_markdown_with_images(
            &result(),
            RenderOptions {
                frontmatter: false,
                image_mode: ImageMode::Embed,
                heading_offset: 0,
                source_url: Some(&base),
                base_url: None,
                retrieved_at: "ignored",
                max_output_bytes: Some(fallback_limit),
            },
            &fetcher,
        )
        .await
        .unwrap();

        assert!(
            embedded
                .content
                .contains("[rdbl-1]: <data:image/png;base64,")
        );
        assert!(
            fallback
                .content
                .contains("[rdbl-1]: <https://example.com/photo.png>")
        );
        assert!(fallback.content.len() <= fallback_limit);
    }

    #[test]
    fn image_mode_uses_standard_string_traits() {
        assert_eq!("embed".parse(), Ok(ImageMode::Embed));
        assert_eq!(ImageMode::Link.to_string(), "link");
        assert_eq!(
            "inline".parse::<ImageMode>().unwrap_err().to_string(),
            "invalid image mode \"inline\"; expected embed, link, or omit"
        );
    }
}
