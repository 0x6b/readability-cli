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

    /// Prompt to use for mods. If not provided, will be selected from a list of prompts specified in the config file.
    /// If you want to use a prompt that contains spaces, you must quote it.
    #[clap(short, long)]
    pub prompt: Option<String>,
}
