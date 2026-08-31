use std::{
    collections::HashMap,
    io::{Read, stdin},
    process::exit,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use clap::Parser;
use html2md::{
    Handle, StructuredPrinter, TagHandler, TagHandlerFactory, parse_html, parse_html_custom,
};
use rdbl_core::{ExtractOptions, ExtractResult, extract};
use reqwest::{Client, Url};
use serde_json::{to_string, to_string_pretty};
use sha2::{Digest, Sha256};

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

    // Get HTML content
    let html = if args.stdin {
        let mut buffer = String::new();
        stdin().read_to_string(&mut buffer)?;
        buffer
    } else if let Some(url) = &args.url {
        fetch_url(url, &args.user_agent).await?
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
                    args.heading_offset,
                    source_url,
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
    heading_offset: u8,
    source_url: Option<&Url>,
    retrieved_at: &str,
) -> String {
    let mut markdown = String::new();
    if let Some(title) = &result.title {
        markdown.push_str(&"#".repeat(shifted_heading_level(1, heading_offset)));
        markdown.push(' ');
        markdown.push_str(title);
        markdown.push_str("\n\n");
    }
    markdown.push_str(&markdown_from_html(&result.content_html, heading_offset));

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

fn markdown_from_html(html: &str, heading_offset: u8) -> String {
    if heading_offset == 0 {
        return parse_html(html);
    }

    let mut handlers: HashMap<String, Box<dyn TagHandlerFactory>> = HashMap::new();
    for level in 1..=6 {
        handlers.insert(
            format!("h{level}"),
            Box::new(ShiftedHeaderFactory {
                level: shifted_heading_level(level, heading_offset),
            }),
        );
    }
    parse_html_custom(html, &handlers)
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
    Ok(format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"))
}

async fn fetch_url(url: &Url, user_agent: &str) -> Result<String> {
    let client = Client::builder().user_agent(user_agent).build()?;

    let response = client.get(url.as_str()).send().await?;

    if !response.status().is_success() {
        bail!("HTTP error: {}", response.status());
    }

    let html = response.text().await?;
    Ok(html)
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
            render_markdown(&result(Some("Article"), None), false, 0, None, "ignored"),
            "# Article\n\nArchive body: 日本語\n"
        );
    }

    #[test]
    fn frontmatter_quotes_special_strings_and_hashes_exact_body() {
        let result = result(Some("題名: \"引用\"\n次の行"), Some("著者: Jane's \"J\""));
        let url = Url::parse("https://example.com/path?q=a:b&name=%E6%97%A5%E6%9C%AC").unwrap();
        let body = "### 題名: \"引用\"\n次の行\n\nArchive body: 日本語";
        let hash = format!("{:x}", Sha256::digest(body.as_bytes()));

        let output = render_markdown(&result, true, 2, Some(&url), "2026-08-31T12:34:56Z");

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
            0,
            None,
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

        let output = render_markdown(&result, false, 2, None, "ignored");

        assert!(output.starts_with("### 題名\n\n### 見出し一"));
        assert!(output.contains("###### Heading five"));
        assert!(output.contains("###### 見出し六"));
        assert!(output.contains("```\n# unchanged\n## code\n```"));
    }

    #[test]
    fn formats_utc_timestamp() {
        let time = UNIX_EPOCH + Duration::from_secs(1_788_179_696);
        assert_eq!(format_utc(time).unwrap(), "2026-08-31T12:34:56Z");
    }
}
