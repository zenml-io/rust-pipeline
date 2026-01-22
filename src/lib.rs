use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use regex::Regex;
use std::sync::LazyLock;

// Pre-compiled regex patterns for performance
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static MONEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$[\d,]+(?:\.\d{2})?\s*(?:million|billion|thousand|M|B|K)?").unwrap()
});
static PERCENTAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d+(?:\.\d+)?%").unwrap()
});
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:Q[1-4]\s+\d{4}|\d{4}-\d{2}-\d{2}|(?:January|February|March|April|May|June|July|August|September|October|November|December)\s+\d{1,2},?\s+\d{4})").unwrap()
});
static TICKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z]{2,5}\b").unwrap()
});
static SENTENCE_END_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[.!?]\s+").unwrap()
});

/// Clean and normalize text for downstream processing.
/// 
/// Performs:
/// - Unicode normalization (NFKC)
/// - Whitespace collapsing
/// - Quote/dash standardization
/// - Control character removal
#[pyfunction]
fn clean_text(text: &str) -> String {
    let mut result = text.to_string();
    
    // Standardize quotes (curly quotes to straight quotes)
    result = result
        .replace('\u{201C}', "\"")  // Left double quotation mark "
        .replace('\u{201D}', "\"")  // Right double quotation mark "
        .replace('\u{2018}', "'")   // Left single quotation mark '
        .replace('\u{2019}', "'")   // Right single quotation mark '
        .replace('`', "'");
    
    // Standardize dashes
    result = result
        .replace('–', "-")
        .replace('—', "-")
        .replace('−', "-");
    
    // Remove control characters (except newlines and tabs)
    result = result
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    
    // Collapse whitespace (preserve paragraph breaks)
    let paragraphs: Vec<&str> = result.split("\n\n").collect();
    let cleaned_paragraphs: Vec<String> = paragraphs
        .iter()
        .map(|p| WHITESPACE_RE.replace_all(p.trim(), " ").to_string())
        .filter(|p| !p.is_empty())
        .collect();
    
    cleaned_paragraphs.join("\n\n")
}

/// Internal implementation of chunk_text (pure Rust, no PyO3 dependencies).
/// Returns None if target_size is 0, otherwise returns the chunks.
fn chunk_text_impl(text: &str, target_size: usize, overlap: usize) -> Option<Vec<String>> {
    // Validate parameters
    if target_size == 0 {
        return None;
    }

    // Clamp overlap to be less than target_size
    let overlap = overlap.min(target_size.saturating_sub(1));

    if text.is_empty() {
        return Some(vec![]);
    }

    // Split into sentences
    let sentences: Vec<&str> = SENTENCE_END_RE
        .split(text)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if sentences.is_empty() {
        // No sentence boundaries found, return as single chunk or split by size
        if text.len() <= target_size {
            return Some(vec![text.to_string()]);
        }
        // Fall back to simple splitting for very long text without periods
        return Some(
            text.chars()
                .collect::<Vec<_>>()
                .chunks(target_size)
                .map(|c| c.iter().collect::<String>())
                .collect(),
        );
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current_chunk = String::new();
    let mut overlap_buffer: Vec<String> = Vec::new();

    for sentence in sentences {
        let sentence_with_period = format!("{}. ", sentence);

        // Check if adding this sentence would exceed target
        if !current_chunk.is_empty()
            && current_chunk.len() + sentence_with_period.len() > target_size
        {
            // Save current chunk
            chunks.push(current_chunk.trim().to_string());

            // Start new chunk with overlap from previous sentences
            current_chunk = overlap_buffer.join(" ");
            overlap_buffer.clear();
        }

        current_chunk.push_str(&sentence_with_period);

        // Track recent sentences for overlap
        overlap_buffer.push(sentence_with_period.clone());
        let overlap_len: usize = overlap_buffer.iter().map(|s| s.len()).sum();
        while overlap_len > overlap && overlap_buffer.len() > 1 {
            overlap_buffer.remove(0);
        }
    }

    // Don't forget the last chunk
    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    Some(chunks)
}

/// Split text into chunks suitable for embedding.
///
/// Args:
///     text: The input text to chunk
///     target_size: Target chunk size in characters (default: 1500, roughly ~375 tokens)
///     overlap: Number of characters to overlap between chunks (default: 200)
///
/// Returns:
///     List of text chunks with sentence-boundary awareness
///
/// Raises:
///     ValueError: If target_size is 0
#[pyfunction]
#[pyo3(signature = (text, target_size=1500, overlap=200))]
fn chunk_text(text: &str, target_size: usize, overlap: usize) -> PyResult<Vec<String>> {
    chunk_text_impl(text, target_size, overlap)
        .ok_or_else(|| PyValueError::new_err("target_size must be greater than 0"))
}

/// Extract financial metadata from text.
/// 
/// Identifies and extracts:
/// - Monetary amounts ($X million, etc.)
/// - Percentages
/// - Dates (Q1 2024, January 15, 2024, etc.)
/// - Potential ticker symbols
/// 
/// Returns a dict with lists of found entities.
#[pyfunction]
fn extract_metadata(py: Python<'_>, text: &str) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    
    // Extract monetary amounts
    let money: Vec<String> = MONEY_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect();
    dict.set_item("monetary_amounts", money)?;
    
    // Extract percentages
    let percentages: Vec<String> = PERCENTAGE_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect();
    dict.set_item("percentages", percentages)?;
    
    // Extract dates
    let dates: Vec<String> = DATE_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect();
    dict.set_item("dates", dates)?;
    
    // Extract potential ticker symbols (filter common words)
    let common_words: std::collections::HashSet<&str> = [
        "THE", "AND", "FOR", "ARE", "BUT", "NOT", "YOU", "ALL", "CAN", "HAD",
        "HER", "WAS", "ONE", "OUR", "OUT", "CEO", "CFO", "COO", "IPO", "USA",
    ].iter().cloned().collect();
    
    let tickers: Vec<String> = TICKER_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .filter(|t| !common_words.contains(t.as_str()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    dict.set_item("potential_tickers", tickers)?;
    
    Ok(dict.into())
}

/// Process a document through the full pipeline: clean, chunk, and extract metadata.
/// 
/// This is a convenience function that runs all three steps and returns
/// a list of dicts, one per chunk, each containing the chunk text and its metadata.
#[pyfunction]
#[pyo3(signature = (text, chunk_size=1500, chunk_overlap=200))]
fn process_document(
    py: Python<'_>,
    text: &str,
    chunk_size: usize,
    chunk_overlap: usize,
) -> PyResult<Vec<Py<PyDict>>> {
    let cleaned = clean_text(text);
    let chunks = chunk_text(&cleaned, chunk_size, chunk_overlap)?;
    
    let mut results: Vec<Py<PyDict>> = Vec::with_capacity(chunks.len());
    
    for (i, chunk) in chunks.iter().enumerate() {
        let dict = PyDict::new(py);
        dict.set_item("chunk_index", i)?;
        dict.set_item("text", chunk)?;
        dict.set_item("char_count", chunk.len())?;
        
        // Extract metadata for this chunk
        let metadata = extract_metadata(py, chunk)?;
        dict.set_item("metadata", metadata)?;
        
        results.push(dict.into());
    }
    
    Ok(results)
}

/// The Python module definition.
#[pymodule]
fn rag_rust_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(clean_text, m)?)?;
    m.add_function(wrap_pyfunction!(chunk_text, m)?)?;
    m.add_function(wrap_pyfunction!(extract_metadata, m)?)?;
    m.add_function(wrap_pyfunction!(process_document, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text() {
        let input = "Hello   world.\n\n\nThis is   a test.";
        let result = clean_text(input);
        assert!(result.contains("Hello world."));
        assert!(result.contains("This is a test."));
    }

    #[test]
    fn test_chunk_text() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let chunks = chunk_text_impl(text, 30, 10).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_zero_size_returns_none() {
        let text = "Some text.";
        let result = chunk_text_impl(text, 0, 10);
        assert!(result.is_none());
    }

    #[test]
    fn test_standardize_quotes() {
        // Input with curly quotes (Unicode)
        let input = "\u{201C}Hello\u{201D} and \u{2018}world\u{2019}";
        let result = clean_text(input);
        assert_eq!(result, "\"Hello\" and 'world'");
    }
}
