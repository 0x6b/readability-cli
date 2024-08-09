use clap::Parser;

use crate::model::Model;

#[derive(Parser)]
#[clap(version, about)]
pub struct Args {
    /// A URL to fetch
    pub url: String,

    /// Display prompts selection and ask OpenAI to process the article with the prompt.
    #[clap(short, long)]
    pub summary: bool,

    /// Model to use. `4o` - gpt-4o, `mini` - gpt-4o-mini, `4` - gpt-4-turbo, `35` - gpt-3.5-turbo
    #[arg(short, long, default_value = "mini")]
    pub model: Model,

    /// Prompt to use for summarization. Default embedded prompt will be used if none is provided.
    /// If you want to use a prompt that contains spaces, you must quote it.
    #[clap(short, long)]
    pub prompt: Option<String>,

    /// Language to use for summarization.
    #[clap(short, long, default_value = "Japanese")]
    pub language: String,
}
