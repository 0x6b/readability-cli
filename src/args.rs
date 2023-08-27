use crate::model::Model;
use clap::Parser;

#[derive(Parser)]
#[clap(version, about)]
pub struct Args {
    /// A URL to fetch
    pub url: String,

    /// Display prompts selection and ask OpenAI to process the article with the prompt, by running mods, which should be in your PATH.
    #[clap(short, long)]
    pub summary: bool,

    /// Model to use for mods. Available models and its aliases:
    ///   gpt-4 (4),
    ///   gpt-4-32k (32k),
    ///   gpt-3.5-turbo (35t)
    #[clap(short, long, default_value = "gpt-3.5-turbo")]
    pub model: Model,
}
