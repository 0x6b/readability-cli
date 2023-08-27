# readability-cli

A naive CLI to fetch a webpage from a URL provided, extract the main content with [readable-app/readability.rs](https://github.com/readable-app/readability.rs), then convert the HTML to markdown, and dump it to the stdout.

## Installation

```console
$ cargo install --git https://github.com/0x6b/readability-cli
```

The binary will be installed to `~/.cargo/bin/rdbl`.

Install [charmbracelet/mods](https://github.com/charmbracelet/mods) if you want to use the `-s` option.

## Usage

```console
$ rdbl --help
A CLI to fetch a webpage from a URL provided, extract the main content, then convert the HTML to markdown, and dump it to the stdout.

Usage: rdbl [OPTIONS] <URL>

Arguments:
  <URL>  A URL to fetch

Options:
  -s, --summary          Display prompts selection and ask OpenAI to process the article with the prompt, by running mods, which should be in your PATH
  -m, --model <MODEL>    Model to use for mods. Available models and its aliases: gpt-4 (4), gpt-4-32k (32k), gpt-3.5-turbo (35t) [default: gpt-3.5-turbo]
  -p, --prompt <PROMPT>  Prompt to use for mods. If not provided, will be selected from a list of prompts specified in the config file. If you want to use a prompt that contains spaces, you must quote it
  -h, --help             Print help
  -V, --version          Print version
```

## Configuration

The command will look for a configuration file at `$XDG_CONFIG_HOME/rdbl/config.toml` e.g. `~/.config/rdbl/config.toml` in UNIX-like systems. The configuration file is optional.

```toml
# User agent for requests
user_agent = "..."

# Collection of prompts to use for the summarization task
prompts = [
    "...",
    "...",
]
```

See [`config.example.toml`](config.example.toml).

## Privacy

The process which converts the HTML to markdown is solely done locally. However, note that:

- your IP address, etc. will be visible to the website from which you fetch the content. This is a standard internet protocol over which I have no control.
- the content fetched from the website will be sent to OpenAI or the server where you configured to run the `mods`, if you use the `-s` option.

## License

MIT. See [LICENSE](LICENSE) for details.
