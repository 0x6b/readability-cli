use std::{
    error::Error,
    io::Write,
    process::{Command, Stdio},
    str::from_utf8,
};

use clap::Parser;
use inquire::Select;
use reqwest::header;
use xdg::BaseDirectories;

use crate::{args::Args, config::Configuration};

mod args;
mod config;
mod model;

fn main() -> Result<(), Box<dyn Error>> {
    let Args {
        url,
        summary: mods,
        model,
        prompt,
    } = Args::parse();

    let config = get_config()?;
    let (title, markdown) = get_content(&url, &config)?;

    println!("# {title}\n\n{}", &markdown);

    if mods {
        println!("---");
        let prompt = match prompt {
            Some(p) => p,
            None => match Select::new("Your prompt? (Ctrl-c to just summarize)", config.prompts)
                .prompt()
            {
                Ok(s) => s,
                Err(_) => "In a few sentences, summarize the key ideas presented in this article"
                    .to_string(),
            },
        };

        let mut child = Command::new("mods")
            .arg("--quiet")
            .arg("--format")
            .arg(format!("--model={}", model))
            .arg(&prompt)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        std::thread::spawn(move || {
            writeln!(stdin, "{}", markdown).unwrap();
        });

        let output = child.wait_with_output()?;
        println!("# Summary\n");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        println!("{}", String::from_utf8_lossy(&output.stderr));
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

fn get_content(url: &str, config: &Configuration) -> Result<(String, String), Box<dyn Error>> {
    let content = reqwest::blocking::Client::new()
        .get(url)
        .header(header::USER_AGENT, &config.user_agent)
        .send()?;
    let text = content.text()?;
    let (nodes, metadata) = readable_readability::Readability::new()
        .base_url(reqwest::Url::parse(url)?)
        .parse(&text);
    let mut text = vec![];
    nodes.serialize(&mut text)?;

    let title = metadata.page_title.unwrap_or("(no title)".to_string());
    let markdown = html2md::parse_html(from_utf8(&text)?);

    Ok((title, markdown))
}
