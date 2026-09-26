use std::{
    io::{Read, stdin},
    process::exit,
    time::SystemTime,
};

use anyhow::{Result, bail};
use clap::{
    Parser,
    builder::{PossibleValuesParser, TypedValueParser},
};
use rdbl::{
    ExtractOptions, HtmlFetcher, ImageMode, RenderOptions, ReqwestFetcher, extract, format_utc,
    render_markdown_with_images,
};
use reqwest::{Client, Url};
use serde_json::to_string_pretty;

#[derive(Parser)]
#[clap(version, about = "Extract readable content from HTML")]
struct Args {
    /// URL to fetch and extract content from
    url: Option<Url>,
    /// Read HTML from stdin instead of URL
    #[clap(long, short)]
    stdin: bool,
    /// Output format: markdown, html, text, json
    #[clap(long, short, default_value = "markdown")]
    format: String,
    /// Prepend archive metadata as YAML frontmatter (markdown only)
    #[clap(long)]
    frontmatter: bool,
    /// Image handling in Markdown: embed, link, or omit
    #[clap(long, default_value_t, value_parser = image_mode_parser())]
    image_mode: ImageMode,
    /// Increase Markdown heading levels, clamping at h6
    #[clap(long, default_value = "0")]
    heading_offset: u8,
    /// Minimum text characters for content (default: 200)
    #[clap(long, default_value = "200")]
    min_text: usize,
    /// Enable debug output
    #[clap(long)]
    debug: bool,
    /// User agent string for HTTP requests
    #[clap(
        long,
        default_value = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:139.0) Gecko/20100101 Firefox/139.0"
    )]
    user_agent: String,
}

fn image_mode_parser() -> impl TypedValueParser<Value = ImageMode> {
    PossibleValuesParser::new(["embed", "link", "omit"]).map(|value| {
        value
            .parse()
            .expect("all possible values are valid image modes")
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.format != "markdown" && (args.frontmatter || args.heading_offset > 0) {
        bail!("--frontmatter and --heading-offset can only be used with --format markdown");
    }
    let client = Client::builder().user_agent(&args.user_agent).build()?;
    let fetcher = ReqwestFetcher::new(client);
    let (html, base_url) = if args.stdin {
        let mut buffer = String::new();
        stdin().read_to_string(&mut buffer)?;
        (buffer, None)
    } else if let Some(url) = &args.url {
        let fetched = fetcher.fetch_html(url).await?;
        (fetched.html, Some(fetched.final_url))
    } else {
        eprintln!("Error: Either provide a URL or use --stdin to read HTML from stdin");
        exit(1);
    };
    let retrieved_at = format_utc(SystemTime::now())?;
    let result = extract(
        &html,
        &ExtractOptions {
            min_text_chars: args.min_text,
            debug: args.debug,
            ..Default::default()
        },
    );
    match args.format.as_str() {
        "json" => println!("{}", to_string_pretty(&result)?),
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
        _ => print!(
            "{}",
            render_markdown_with_images(
                &result,
                RenderOptions {
                    frontmatter: args.frontmatter,
                    image_mode: args.image_mode,
                    heading_offset: args.heading_offset,
                    source_url: (!args.stdin).then_some(args.url.as_ref()).flatten(),
                    base_url: base_url.as_ref(),
                    retrieved_at: &retrieved_at,
                    max_output_bytes: None,
                },
                &fetcher,
            )
            .await?
        ),
    }
    Ok(())
}
