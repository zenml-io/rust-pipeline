"""ZenML steps that wrap Rust text processing functions."""

from pathlib import Path
from typing import Annotated

from zenml import step, log_artifact_metadata

# Import our Rust module (built via maturin)
import rag_rust_core


@step
def load_documents(
    data_dir: str = "data/sample_transcripts",
) -> Annotated[list[dict[str, str]], "documents"]:
    """Load documents from a directory.
    
    Args:
        data_dir: Path to directory containing .txt files
        
    Returns:
        List of dicts with 'filename' and 'content' keys
    """
    data_path = Path(data_dir)
    documents = []
    
    for file_path in sorted(data_path.glob("*.txt")):
        content = file_path.read_text(encoding="utf-8")
        documents.append({
            "filename": file_path.name,
            "content": content,
        })
    
    log_artifact_metadata(
        metadata={
            "document_count": len(documents),
            "total_chars": sum(len(d["content"]) for d in documents),
        }
    )
    
    return documents


@step
def process_documents(
    documents: list[dict[str, str]],
    chunk_size: int = 1500,
    chunk_overlap: int = 200,
) -> Annotated[list[dict], "processed_chunks"]:
    """Process documents through the Rust text pipeline.
    
    For each document:
    1. Clean and normalize text (Rust)
    2. Split into chunks (Rust)
    3. Extract metadata from each chunk (Rust)
    
    Args:
        documents: List of documents from load_documents step
        chunk_size: Target chunk size in characters
        chunk_overlap: Overlap between chunks in characters
        
    Returns:
        List of processed chunks with text and metadata
    """
    all_chunks = []
    
    for doc in documents:
        # Call the Rust function that does all three steps
        chunks = rag_rust_core.process_document(
            doc["content"],
            chunk_size=chunk_size,
            chunk_overlap=chunk_overlap,
        )
        
        # Add source document info to each chunk
        for chunk in chunks:
            chunk["source_file"] = doc["filename"]
            all_chunks.append(chunk)
    
    log_artifact_metadata(
        metadata={
            "total_chunks": len(all_chunks),
            "avg_chunk_size": (
                sum(c["char_count"] for c in all_chunks) / len(all_chunks)
                if all_chunks else 0
            ),
            "documents_processed": len(documents),
        }
    )
    
    return all_chunks


@step
def save_results(
    chunks: list[dict],
    output_path: str = "output/processed_chunks.json",
) -> Annotated[str, "output_file"]:
    """Save processed chunks to JSON.
    
    Args:
        chunks: Processed chunks from process_documents step
        output_path: Where to save the JSON output
        
    Returns:
        Path to the saved file
    """
    import json
    
    output = Path(output_path)
    output.parent.mkdir(parents=True, exist_ok=True)
    
    with open(output, "w", encoding="utf-8") as f:
        json.dump(chunks, f, indent=2, ensure_ascii=False)
    
    log_artifact_metadata(
        metadata={
            "output_file": str(output.absolute()),
            "file_size_bytes": output.stat().st_size,
        }
    )
    
    return str(output.absolute())
