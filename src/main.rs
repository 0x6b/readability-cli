use std::str::from_utf8;

use clap::Parser;

#[derive(Parser)]
#[clap(version, about)]
struct Args {
    /// A URL to fetch
    url: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args { url } = Args::parse();

    let content = reqwest::blocking::get(&url)?;
    let text = content.text()?;
    let (nodes, metadata) = readable_readability::Readability::new()
        .base_url(reqwest::Url::parse(&url)?)
        .parse(&text);
    let mut text = vec![];
    nodes.serialize(&mut text)?;

    let title = metadata.page_title.unwrap_or("(no title)".to_string());

    println!("# {title}\n\n{}", html2md::parse_html(from_utf8(&text)?));

    Ok(())
}
