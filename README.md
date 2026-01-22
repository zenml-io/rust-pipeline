# Rust-Powered RAG Preprocessing with ZenML

This example demonstrates how to use **Rust** for high-performance text processing within a **ZenML** pipeline, using [PyO3](https://pyo3.rs) to expose Rust functions to Python.

<img width="1531" height="966" alt="CleanShot 2026-01-22 at 10 14 53" src="https://github.com/user-attachments/assets/4e558197-e1b8-4210-8601-2244613256c2" />

## What This Does

Processes financial documents (earnings call transcripts) to prepare them for a RAG system:

1. **Load documents** — Read `.txt` files from a directory
2. **Process with Rust** — Clean text, chunk with sentence-boundary awareness, extract metadata
3. **Save results** — Output processed chunks as JSON

The text processing is implemented in Rust for:
- Unicode normalization and text cleanup
- Sentence-aware chunking with configurable overlap
- Metadata extraction (dates, monetary amounts, percentages, ticker symbols)

## Prerequisites

- **Rust** — Install via [rustup.rs](https://rustup.rs)
- **Python 3.9+**
- **uv** — Install via [docs.astral.sh/uv](https://docs.astral.sh/uv/)

## Quick Start

```bash
# Install and build
make install

# Run the pipeline
make run
```

Or step by step:

```bash
uv sync --extra dev          # Install Python + dev dependencies (includes maturin)
uv run maturin develop       # Build Rust → Python module
uv run python run.py         # Run pipeline
```

## Project Structure

```
├── src/lib.rs              # Rust text processing functions
├── Cargo.toml              # Rust dependencies
├── pyproject.toml          # Python/maturin config
├── rag_preprocessing/
│   ├── steps.py            # ZenML steps (thin wrappers)
│   └── pipeline.py         # Pipeline definition
├── data/sample_transcripts/ # Sample financial documents
└── run.py                  # Entry point
```

## How It Works

### The Rust Side

PyO3 lets you expose Rust functions to Python with annotations:

```rust
#[pyfunction]
fn clean_text(text: &str) -> String {
    // Rust text processing logic
}

#[pymodule]
fn rag_rust_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(clean_text, m)?)?;
    Ok(())
}
```

Running `maturin develop` compiles this into a Python module you can import normally.

### The ZenML Side

ZenML steps just import and call the Rust module:

```python
import rag_rust_core  # This is our compiled Rust

@step
def process_documents(documents: list[dict]) -> list[dict]:
    all_chunks = []
    for doc in documents:
        # Call Rust function like any Python function
        chunks = rag_rust_core.process_document(doc["content"])
        all_chunks.extend(chunks)
    return all_chunks
```

ZenML handles orchestration, caching, artifact tracking, and observability — the Rust code is invisible to it.

## Available Rust Functions

| Function | Description |
|----------|-------------|
| `clean_text(text)` | Normalize unicode, collapse whitespace, standardize quotes/dashes |
| `chunk_text(text, size, overlap)` | Split into chunks respecting sentence boundaries |
| `extract_metadata(text)` | Extract dates, amounts, percentages, tickers |
| `process_document(text, size, overlap)` | All-in-one: clean → chunk → extract |

## Configuration

```bash
uv run python run.py --help

# Custom chunk size
uv run python run.py --chunk-size 1000 --chunk-overlap 100

# Different data directory
uv run python run.py --data-dir /path/to/documents
```

## Running Rust Tests

```bash
cargo test
```

## ZenML Stack Notes

This demo is designed for the **default local ZenML stack**. If you have a remote stack configured (e.g., with an S3 artifact store) and encounter errors, switch back to the local stack with `zenml stack set default`.

**Running on cloud orchestrators:** To run this pipeline on Kubernetes, Vertex AI, or other remote orchestrators, you'll need to ensure the compiled Rust extension is available in your step's Docker image. The simplest approach is to build a custom parent image that includes the Rust toolchain and pre-built wheel, then reference it via ZenML's `DockerSettings`. You can specify `apt_packages` for system dependencies, use a custom `dockerfile` for full control over the build, or point to a pre-built `parent_image` with `skip_build=True` if you've already baked in ZenML and the extension. See ZenML's [containerization docs](https://docs.zenml.io/concepts/containerization) for details. You'll also want to use cloud URIs (like `s3://...`) for your data paths, or bake the sample data into your image.

## Why This Approach?

For Rust developers who want MLOps tooling:
- Write idiomatic Rust with normal tooling (`cargo`, tests, lints)
- PyO3 handles Python↔Rust type conversion
- ZenML provides orchestration, caching, lineage tracking, dashboard
- Your Rust code runs unmodified — no special adaptation needed

## Resources

- [PyO3 User Guide](https://pyo3.rs)
- [maturin](https://github.com/PyO3/maturin)
- [ZenML Docs](https://docs.zenml.io)
