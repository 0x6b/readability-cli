# Training Tools

Tools for training the logistic regression model used by `readable_core`.

## Setup

```bash
cd tools
pnpm install  # Install Node.js dependencies (Readability.js)
uv sync       # Install Python dependencies
```

## Building a Corpus

### From URLs

Save web pages to the corpus directory:

```bash
# Save with auto-generated filename
uv run save_corpus.py https://example.com/article

# Save with custom name
uv run save_corpus.py https://example.com/article --name my_article
```

### From Mozilla Readability tests

Import test cases from the [Mozilla Readability](https://github.com/mozilla/readability) repository:

```bash
# Clone or download readability repo, then:
uv run import_mozilla_tests.py /path/to/readability/test/test-pages
```

This imports both `source.html` and `expected.html` files, using the expected output as ground truth labels.

### Corpus format

- `{name}.html` - Source HTML file
- `{name}.expected.html` - (Optional) Expected extracted content for labeling

If `.expected.html` exists, it's used as the teacher output. Otherwise, Readability.js is invoked.

## Training the Model

```bash
uv run train_logreg.py --corpus ../tests/corpus --output weights.json
```

Options:
- `--corpus` - Directory containing HTML training files (default: `tests/corpus`)
- `--output` - Output file for trained weights (default: `model_weights.json`)
- `--C` - Regularization parameter (default: 1.0)

## Exporting Weights to Rust

```bash
uv run export_weights.py --input weights.json --output ../crates/readable_core/src/model.rs
```

This generates a complete `model.rs` file with trained weights.
