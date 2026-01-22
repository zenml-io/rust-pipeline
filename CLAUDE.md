# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **Rust + Python hybrid project** demonstrating high-performance text processing for RAG (Retrieval-Augmented Generation) pipelines. Rust handles CPU-intensive text operations via PyO3, while ZenML provides Python-based MLOps orchestration.

## Build & Development Commands

```bash
# Initial setup (installs Python deps + builds Rust module)
make install

# Rebuild Rust module after code changes
make build                    # Development build
make build-release           # Optimized build

# Run the pipeline
make run                     # Default settings
uv run python run.py --chunk-size 1000 --chunk-overlap 150  # Custom params

# Rust development
cargo test                   # Run Rust tests
cargo test -- --nocapture    # With stdout
cargo check                  # Fast compile check (no codegen)
cargo clippy                 # Lint
cargo fmt                    # Format

# Cleanup
make clean
```

## Architecture

### Two-Language Bridge Pattern

```
┌─────────────────────────────────────────────────────────────┐
│  Python Layer (ZenML)                                       │
│  ├── run.py            Entry point with CLI                 │
│  └── rag_preprocessing/                                     │
│      ├── pipeline.py   Pipeline definition (3 steps)        │
│      └── steps.py      ZenML steps (thin wrappers)          │
│                            │                                │
│                            ▼ imports rag_rust_core          │
├─────────────────────────────────────────────────────────────┤
│  Rust Layer (PyO3)                                          │
│  └── src/lib.rs        Text processing functions            │
│      ├── clean_text()       Unicode normalization           │
│      ├── chunk_text()       Sentence-aware splitting        │
│      ├── extract_metadata() Financial entity extraction     │
│      └── process_document() All-in-one pipeline             │
└─────────────────────────────────────────────────────────────┘
```

**Key insight**: The Rust code compiles to a Python module (`rag_rust_core`) via maturin. ZenML steps import and call Rust functions like normal Python—ZenML is unaware that Rust is involved.

### Data Flow

1. `load_documents` → reads `.txt` files from `data/sample_transcripts/`
2. `process_documents` → calls Rust's `process_document()` for each file
3. `save_results` → writes JSON to `output/`

### Rust Module Structure

The `src/lib.rs` file uses:
- `LazyLock` for pre-compiled regex patterns (zero runtime compilation cost)
- `#[pyfunction]` to expose functions to Python
- `#[pymodule]` to define the module interface

## Critical Development Notes

1. **After any Rust code changes**, you must rebuild: `make build` or `uv run maturin develop`
2. **The Rust module name** (`rag_rust_core`) is configured in both `Cargo.toml` and `pyproject.toml`—keep them in sync
3. **Maturin config** in `pyproject.toml` sets `python-source = "."` meaning the module is importable from the project root after build
