# Training Tools

Tools for training the logistic regression model used by `readable_core`.

## Setup

```bash
cd tools
pnpm install  # Install Node.js dependencies (Readability.js)
uv sync       # Install Python dependencies
```

## Building a Corpus

Save web pages to the corpus directory for training:

```bash
# Save with auto-generated filename
uv run save_corpus.py https://example.com/article

# Save with custom name
uv run save_corpus.py https://example.com/article --name my_article

# Save to custom directory
uv run save_corpus.py https://example.com/article --corpus /path/to/corpus
```

Corpus files are saved to `tests/corpus/` by default.

## Training the Model

Train a logistic regression model using Readability.js as a teacher:

```bash
uv run train_logreg.py --corpus ../tests/corpus --output model_weights.json
```

Options:
- `--corpus` - Directory containing HTML training files (default: `tests/corpus`)
- `--output` - Output file for trained weights (default: `model_weights.json`)
- `--C` - Regularization parameter (default: 1.0)

## Exporting Weights to Rust

Convert trained weights to Rust const arrays:

```bash
uv run export_weights.py model_weights.json ../crates/readable_core/src/model.rs
```

This updates the `WEIGHTS` and `BIAS` constants in `model.rs`.
