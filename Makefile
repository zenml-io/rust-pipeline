.PHONY: build install run test clean help

# Default target
help:
	@echo "Rust + PyO3 + ZenML RAG Preprocessing Example"
	@echo ""
	@echo "Targets:"
	@echo "  build     Build the Rust module (development mode)"
	@echo "  install   Install dependencies and build"
	@echo "  run       Run the ZenML pipeline"
	@echo "  test      Run Rust tests"
	@echo "  clean     Remove build artifacts"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - Rust toolchain (rustup.rs)"
	@echo "  - Python 3.9+"
	@echo "  - uv (https://docs.astral.sh/uv/)"

# Build the Rust module in development mode
build:
	uv run maturin develop

# Build with release optimizations
build-release:
	uv run maturin develop --release

# Install all dependencies and build
install:
	uv sync
	uv run maturin develop

# Run the pipeline
run: build
	uv run python run.py

# Run with custom options
run-custom: build
	uv run python run.py --chunk-size 1000 --chunk-overlap 150

# Run Rust tests
test:
	cargo test

# Run Rust tests with output
test-verbose:
	cargo test -- --nocapture

# Clean build artifacts (excludes virtual environments)
clean:
	cargo clean
	rm -rf target/
	rm -rf output/
	rm -rf *.egg-info/
	rm -rf __pycache__/
	rm -rf rag_preprocessing/__pycache__/
	find . -path "./.venv" -prune -o -path "./venv" -prune -o -name "*.so" -type f -print -delete
	find . -path "./.venv" -prune -o -path "./venv" -prune -o -name "*.pyd" -type f -print -delete

# Format Rust code
fmt:
	cargo fmt

# Check Rust code without building
check:
	cargo check

# Lint Rust code
lint:
	cargo clippy
