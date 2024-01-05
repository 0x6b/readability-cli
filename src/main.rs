use std::{error::Error, str::from_utf8};

use async_openai::{
    types::{ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs},
    Client,
};
use clap::Parser;
use futures::StreamExt;
use reqwest::header;
use xdg::BaseDirectories;

use crate::{args::Args, config::Configuration};

mod args;
mod config;
mod model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let Args { url, summary, model, prompt, language } = Args::parse();

    let config = get_config()?;
    let (title, markdown) = get_content(&url, &config).await?;

    println!("{}", termimad::text(&format!("# {title}\n\n{}\n", &markdown)));

    if summary {
        let client = Client::new();
        let request = async_openai::types::CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model(model.to_string())
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(
                        if let Some(prompt) = prompt {
                            prompt
                        } else {
                            format!(r#"As a professional summarizer, create a concise and comprehensive summary of the provided text, be it an article, post, conversation, or passage, while adhering to these guidelines:

1. In 3 paragraphs or less, craft a summary that is detailed, thorough, in-depth, and complex, while maintaining clarity and conciseness, in {language}.
2. Incorporate main ideas and essential information, eliminating extraneous language and focusing on critical aspects.
3. Rely strictly on the provided text, without including external information.
4. Utilize markdown to cleanly format your output. Example: **Bold** key subject matter and potential areas that may need expanded information
"#)
                        })
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(markdown.clone())
                    .build()?
                    .into(),
            ])
            .build()?;

        println!("{}", termimad::text("# Summary\n"));
        let stream = client.chat().create_stream(request).await?;
        let result = stream
            .map(|result| {
                let response = result.unwrap();
                let summary = response
                    .choices
                    .iter()
                    .filter_map(|choice| choice.clone().delta.content)
                    .collect::<Vec<String>>()
                    .join("");
                summary
            })
            .collect::<Vec<String>>()
            .await
            .join("");
        println!("{}", termimad::text(&format!("{}\n", result)));
    }

    Ok(())
}

fn get_config() -> Result<Configuration, Box<dyn Error>> {
    let config_path = BaseDirectories::with_prefix("rdbl")?.place_config_file("config.toml")?;
    match std::fs::read_to_string(config_path) {
        Ok(c) => Ok(toml::from_str::<Configuration>(&c)?),
        Err(_) => Ok(Configuration::default()),
    }
}

async fn get_content(
    url: &str,
    config: &Configuration,
) -> Result<(String, String), Box<dyn Error>> {
    let content = reqwest::Client::new()
        .get(url)
        .header(header::USER_AGENT, &config.user_agent)
        .send()
        .await?;
    let text = content.text().await?;
    let (nodes, metadata) = readable_readability::Readability::new()
        .base_url(reqwest::Url::parse(url)?)
        .parse(&text);
    let mut text = vec![];
    nodes.serialize(&mut text)?;

    let title = metadata.page_title.unwrap_or("(no title)".to_string());
    let markdown = html2md::parse_html(from_utf8(&text)?);

    Ok((title, markdown))
}
