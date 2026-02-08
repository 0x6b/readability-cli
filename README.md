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
A CLI to fetch a webpage from a URL provided, extract the main content, then convert the
HTML to markdown, and dump it to the stdout.

Usage: rdbl [OPTIONS] <URL>

Arguments:
  <URL>  A URL to fetch

Options:
  -h, --help                 Print help
  -V, --version              Print version
```

## Privacy

The process which converts the HTML to markdown is solely done locally. However, note that:

- your IP address, etc. will be visible to the website from which you fetch the content. This is a standard internet protocol over which I have no control.

## License

MIT. See [LICENSE](LICENSE) for details.
