use anyhow::Result;
use clap::Parser;
use readablility_cli::Scraper;
use reqwest::Url;

#[derive(Parser)]
#[clap(version, about)]
pub struct Args {
    /// A URL to fetch
    pub url: Url,

    /// User agent string to use for the request
    #[clap(
        long,
        default_value = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:139.0) Gecko/20100101 Firefox/139.0"
    )]
    pub user_agent: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args { url, user_agent } = Args::parse();
    let body = Scraper::try_new(&user_agent)?.scrape(&url).await?;
    println!("{body}");
    Ok(())
}
