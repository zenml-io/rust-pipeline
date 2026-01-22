#!/usr/bin/env python3
"""Run the RAG preprocessing pipeline.

This script demonstrates a ZenML pipeline that uses Rust (via PyO3) for
high-performance text processing.

Usage:
    python run.py                    # Run with defaults
    python run.py --chunk-size 1000  # Custom chunk size
"""

import argparse
from pathlib import Path

from rag_preprocessing import rag_preprocessing_pipeline


def main():
    parser = argparse.ArgumentParser(
        description="Process financial documents for RAG using Rust + ZenML"
    )
    parser.add_argument(
        "--data-dir",
        type=str,
        default="data/sample_transcripts",
        help="Directory containing .txt files to process",
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=1500,
        help="Target chunk size in characters (default: 1500)",
    )
    parser.add_argument(
        "--chunk-overlap",
        type=int,
        default=200,
        help="Overlap between chunks in characters (default: 200)",
    )
    parser.add_argument(
        "--output",
        type=str,
        default="output/processed_chunks.json",
        help="Output file path for processed chunks",
    )
    
    args = parser.parse_args()
    
    # Verify data directory exists
    data_path = Path(args.data_dir)
    if not data_path.exists():
        print(f"Error: Data directory '{args.data_dir}' not found.")
        print("Make sure you're running from the project root directory.")
        return 1
    
    txt_files = list(data_path.glob("*.txt"))
    if not txt_files:
        print(f"Error: No .txt files found in '{args.data_dir}'.")
        return 1
    
    print(f"Found {len(txt_files)} documents to process")
    print(f"Chunk size: {args.chunk_size} chars, overlap: {args.chunk_overlap} chars")
    print()
    
    # Run the pipeline (in ZenML >=0.40, calling the pipeline function executes it)
    rag_preprocessing_pipeline(
        data_dir=args.data_dir,
        chunk_size=args.chunk_size,
        chunk_overlap=args.chunk_overlap,
        output_path=args.output,
    )
    
    print()
    print(f"Done! Output saved to: {args.output}")
    return 0


if __name__ == "__main__":
    exit(main())
