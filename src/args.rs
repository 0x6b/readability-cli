use crate::model::Model;
use clap::Parser;

#[derive(Parser)]
#[clap(version, about)]
pub struct Args {
    /// A URL to fetch
    pub url: String,

    /// Print summary of the input with running mods. mods should be in your PATH.
    #[clap(short, long)]
    pub summary: bool,

    /// Prompt for mods
    #[clap(
        short,
        long,
        default_value = "In a few sentences, summarize the key ideas presented in this article."
    )]
    pub prompt: String,

    /// Model to use for mods. Available models and its aliases:
    ///   gpt-4 (4),
    ///   gpt-4-32k (32k),
    ///   gpt-3.5-turbo (35t)
    #[clap(short, long, default_value = "gpt-3.5-turbo")]
    pub model: Model,

    /// User agent to use for fetching the URL
    #[clap(
        long,
        default_value = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36"
    )]
    pub user_agent: String,
}
