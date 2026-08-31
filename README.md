# rdbl

A CLI to fetch a webpage from a URL provided, extract the main content, then convert the HTML to markdown, and dump it to the stdout.

## Installation

```console
$ cargo install --git https://github.com/0x6b/readability-cli
```

The binary will be installed to `~/.cargo/bin/rdbl`.

## Usage

```console
$ rdbl https://example.com/article
$ cat article.html | rdbl --stdin
$ rdbl --frontmatter https://example.com/article > article.md
$ rdbl --frontmatter --heading-offset 2 https://example.com/article > article.md
```

By default, `rdbl` writes the same plain Markdown output as before. Pass `--frontmatter` to prepend
archive metadata. Frontmatter is supported only with the default `--format markdown`; combining it
with `html`, `text`, or `json` is an error.

```yaml
---
title: "Example article"
byline: "Example Author"
source_url: "https://example.com/article"
retrieved_at: "2026-08-31T12:34:56Z"
generator: "rdbl"
generator_version: "0.10.0"
content_sha256: "f00b..."
---
```

All values are YAML 1.2 double-quoted strings. `byline` is omitted when the page has no author, and
`source_url` is omitted for `--stdin` input. `retrieved_at` is UTC in RFC 3339 format. The SHA-256
is calculated over the exact UTF-8 bytes of the Markdown body after the closing `---` (including an
extracted `# title` heading when present), excluding the frontmatter and the final newline written by
the CLI.

Use `--heading-offset N` when embedding the output below an existing Markdown heading. It increases
both the generated title heading and headings originating from structured `<h1>`–`<h6>` elements by
`N` levels. Levels deeper than h6 are clamped to h6. The shift is applied while converting the
extracted HTML, so Markdown-looking text inside code fences is not changed. This option is also
Markdown-only, and its default of zero preserves the existing output.

```console
$ rdbl --help
Extract readable content from HTML

Usage: rdbl [OPTIONS] [URL]

Arguments:
  [URL]  URL to fetch and extract content from

Options:
  -s, --stdin
          Read HTML from stdin instead of URL
  -f, --format <FORMAT>
          Output format: markdown, html, text, json [default: markdown]
      --frontmatter
          Prepend archive metadata as YAML frontmatter (markdown only)
      --heading-offset <HEADING_OFFSET>
          Increase Markdown heading levels, clamping at h6 [default: 0]
      --min-text <MIN_TEXT>
          Minimum text characters for content (default: 200) [default: 200]
      --debug
          Enable debug output
      --user-agent <USER_AGENT>
          User agent string for HTTP requests [default: "Mozilla/5.0 (Macintosh; Intel Mac OS X
          10.15; rv:139.0) Gecko/20100101 Firefox/139.0"]
  -h, --help
          Print help
  -V, --version
          Print version
```

## Privacy

The process which converts the HTML to markdown is solely done locally. However, note that:

- your IP address, etc. will be visible to the website from which you fetch the content. This is a standard internet protocol over which I have no control.

## License

MIT. See [LICENSE](LICENSE) for details.
