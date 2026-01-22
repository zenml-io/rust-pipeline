"""ZenML pipeline for RAG document preprocessing."""

from zenml import pipeline

from .steps import load_documents, process_documents, save_results


@pipeline
def rag_preprocessing_pipeline(
    data_dir: str = "data/sample_transcripts",
    chunk_size: int = 1500,
    chunk_overlap: int = 200,
    output_path: str = "output/processed_chunks.json",
):
    """Process financial documents for RAG ingestion.
    
    This pipeline demonstrates using Rust (via PyO3) for high-performance
    text processing within a ZenML pipeline.
    
    Pipeline steps:
    1. load_documents: Read .txt files from data_dir
    2. process_documents: Clean, chunk, and extract metadata (Rust)
    3. save_results: Write processed chunks to JSON
    
    Args:
        data_dir: Directory containing source documents
        chunk_size: Target size for text chunks (chars)
        chunk_overlap: Overlap between consecutive chunks (chars)
        output_path: Where to save processed output
    """
    documents = load_documents(data_dir=data_dir)
    chunks = process_documents(
        documents=documents,
        chunk_size=chunk_size,
        chunk_overlap=chunk_overlap,
    )
    save_results(chunks=chunks, output_path=output_path)
