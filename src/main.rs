use std::{
    io::Write,
    process::{Command, Stdio},
    str::from_utf8,
};

use crate::args::Args;
use clap::Parser;
use reqwest::header;

mod args;
mod model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        url,
        summary: mods,
        prompt,
        model,
        user_agent,
    } = Args::parse();

    let content = reqwest::blocking::Client::new()
        .get(&url)
        .header(header::USER_AGENT, user_agent)
        .send()?;
    let text = content.text()?;
    let (nodes, metadata) = readable_readability::Readability::new()
        .base_url(reqwest::Url::parse(&url)?)
        .parse(&text);
    let mut text = vec![];
    nodes.serialize(&mut text)?;

    let title = metadata.page_title.unwrap_or("(no title)".to_string());
    let markdown = html2md::parse_html(from_utf8(&text)?);

    println!("# {title}\n\n{}", &markdown);

    if mods {
        println!("\n---\n");
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
