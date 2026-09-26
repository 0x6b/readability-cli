use std::{
    collections::HashMap,
    io::{Read, stdin},
    process::exit,
    time::SystemTime,
};

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use rdbl::{
    ExtractOptions, ImageMode, RenderOptions, collect_image_sources, extract, format_utc,
    image_data_uri, render_markdown,
};
use reqwest::{Client, Url, header::CONTENT_TYPE};
use serde_json::to_string_pretty;
use tokio::task::JoinSet;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum CliImageMode {
    Embed,
    #[default]
    Link,
    Omit,
}

impl From<CliImageMode> for ImageMode {
    fn from(value: CliImageMode) -> Self {
        match value {
            CliImageMode::Embed => Self::Embed,
            CliImageMode::Link => Self::Link,
            CliImageMode::Omit => Self::Omit,
        }
    }
}

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
    #[clap(long, value_enum, default_value_t)]
    image_mode: CliImageMode,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.format != "markdown" && (args.frontmatter || args.heading_offset > 0) {
        bail!("--frontmatter and --heading-offset can only be used with --format markdown");
    }
    let client = Client::builder().user_agent(&args.user_agent).build()?;
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
    let result = extract(
        &html,
        &ExtractOptions {
            min_text_chars: args.min_text,
            debug: args.debug,
            ..Default::default()
        },
    );
    let image_mode = ImageMode::from(args.image_mode);
    let embedded_images = if image_mode == ImageMode::Embed {
        embed_images(&client, &result.content_html, document_url.as_ref()).await
    } else {
        HashMap::new()
    };

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
            render_markdown(
                &result,
                RenderOptions {
                    frontmatter: args.frontmatter,
                    image_mode,
                    heading_offset: args.heading_offset,
                    source_url: (!args.stdin).then_some(args.url.as_ref()).flatten(),
                    document_url: document_url.as_ref(),
                    embedded_images,
                    retrieved_at: &retrieved_at,
                }
            )
            .content
        ),
    }
    Ok(())
}

async fn embed_images(
    client: &Client,
    html: &str,
    document_url: Option<&Url>,
) -> HashMap<String, String> {
    let sources = collect_image_sources(html, document_url, usize::MAX);
    let mut embedded = HashMap::new();
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
    Some(image_data_uri(&media_type, &bytes))
}

async fn fetch_url(client: &Client, url: &Url) -> Result<(String, Url)> {
    let response = client.get(url.as_str()).send().await?;
    if !response.status().is_success() {
        bail!("HTTP error: {}", response.status());
    }
    let final_url = response.url().clone();
    Ok((response.text().await?, final_url))
}
