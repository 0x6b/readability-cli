# readability-cli

A naive CLI to fetch a webpage from a URL provided, extract the main content with [readable-app/readability.rs](https://github.com/readable-app/readability.rs), then convert the HTML to markdown, and dump it to the stdout.

## Installation

```console
$ cargo install --git https://github.com/0x6b/readability-cli
```

The binary will be installed to `~/.cargo/bin/rdbl`.

## Usage

```console
$ rdbl --help
A CLI to fetch a webpage from a URL provided, extract the main content, then convert the HTML to markdown, and dump it to the stdout.

Usage: rdbl [OPTIONS] <URL>

Arguments:
  <URL>  A URL to fetch

Options:
  -s, --summary              Display prompts selection and ask OpenAI to process the article with the prompt, by running mods, which should be in your PATH
  -m, --model <MODEL>        Model to use. Available models and its aliases: gpt-4 (4) -- gtp-4-1106-preview, gpt-4-32k (32k) -- gpt-4-32k, gpt-3.5-turbo (35t) -- gpt-3.5-turbo-1106 [default: gpt-3.5-turbo]
  -p, --prompt <PROMPT>      Prompt to use for mods. If not provided, will be selected from a list of prompts specified in the config file. If you want to use a prompt that contains spaces, you must quote it
  -l, --language <LANGUAGE>  Language to use for summarization [default: English]
  -h, --help                 Print help
  -V, --version              Print version
```

## Configuration

The command will look for a configuration file at `$XDG_CONFIG_HOME/rdbl/config.toml` e.g. `~/.config/rdbl/config.toml` in UNIX-like systems. The configuration file is optional.

```toml
# User agent for requests
user_agent = "..."
```

See [`config.example.toml`](config.example.toml).

## Privacy

The process which converts the HTML to markdown is solely done locally. However, note that:

- your IP address, etc. will be visible to the website from which you fetch the content. This is a standard internet protocol over which I have no control.
- the content fetched from the website will be sent to OpenAI or the server if you use the `-s` option.

## License

MIT. See [LICENSE](LICENSE) for details.
