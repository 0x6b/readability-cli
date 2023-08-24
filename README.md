# readability-cli

A naive CLI to fetch a webpage from a URL provided, extract the main content with [readable-app/readability.rs](https://github.com/readable-app/readability.rs), then convert the HTML to markdown, and dump it to the stdout.

## Installation

```console
$ cargo install --git https://github.com/0x6b/readability-cli
```

The binary will be installed to `~/.cargo/bin/rdbl`.

Install [charmbracelet/mods](https://github.com/charmbracelet/mods) if you want to use the `-s` option.

## Usage

```bash
$ rdbl --help
A CLI to fetch a webpage from a URL provided, extract the main content, then convert the HTML to markdown, and dump it to the stdout.

Usage: rdbl [OPTIONS] <URL>

Arguments:
  <URL>  A URL to fetch

Options:
  -s, --summary                  Print summary of the input with running mods. mods should be in your PATH
  -p, --prompt <PROMPT>          Prompt for mods [default: "In a few sentences, summarize the key ideas presented in this article."]
  -m, --model <MODEL>            Model to use for mods. Available models and its aliases: gpt-4 (4), gpt-4-32k (32k), gpt-3.5-turbo (35t) [default: gpt-3.5-turbo]
      --user-agent <USER_AGENT>  User agent to use for fetching the URL [default: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36"]
  -h, --help                     Print help
  -V, --version                  Print version
```

## Privacy

The process which converts the HTML to markdown is solely done locally. However, note that:

- your IP address, etc. will be visible to the website from which you fetch the content. This is a standard internet protocol over which I have no control.
- the content fetched from the website will be sent to OpenAI or the server where you configured to run the `mods`, if you use the `-s` option.

## License

MIT. See [LICENSE](LICENSE) for details.
