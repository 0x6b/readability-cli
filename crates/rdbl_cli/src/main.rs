use std::{
    io::{Read, stdin},
    process::exit,
};

use anyhow::{Result, bail};
use clap::Parser;
use html2md::parse_html;
use rdbl_core::{ExtractOptions, extract};
use reqwest::Url;
use serde_json::to_string_pretty;

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
            // Convert HTML to markdown
            if let Some(title) = &result.title {
                println!("# {title}\n");
            }
            let markdown = parse_html(&result.content_html);
            println!("{markdown}");
        }
    }

    Ok(())
}

async fn fetch_url(url: &Url, user_agent: &str) -> Result<String> {
    let client = reqwest::Client::builder().user_agent(user_agent).build()?;

    let response = client.get(url.as_str()).send().await?;

    if !response.status().is_success() {
        bail!("HTTP error: {}", response.status());
    }

    let html = response.text().await?;
    Ok(html)
}
