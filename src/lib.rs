use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

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
// Regex to find sentence-ending punctuation followed by whitespace
static SENTENCE_BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[.!?]+\s+").unwrap()
});

// Static set of common words to filter from ticker detection (avoids per-call allocation)
static COMMON_TICKER_STOPWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "THE", "AND", "FOR", "ARE", "BUT", "NOT", "YOU", "ALL", "CAN", "HAD",
        "HER", "WAS", "ONE", "OUR", "OUT", "CEO", "CFO", "COO", "IPO", "USA",
    ]
    .iter()
    .cloned()
    .collect()
});

/// Helper to count characters (Unicode code points), not bytes.
#[inline]
fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Clean and normalize text for downstream processing.
///
/// Performs:
/// - Unicode normalization (NFKC) - converts compatibility characters to canonical forms
/// - Whitespace collapsing
/// - Quote/dash standardization
/// - Control character removal
#[pyfunction]
fn clean_text(text: &str) -> String {
    // Apply NFKC normalization first (handles compatibility characters like ligatures, fullwidth forms)
    let normalized: String = text.nfkc().collect();

    // Standardize quotes (curly quotes to straight quotes)
    let mut result = normalized
        .replace('\u{201C}', "\"") // Left double quotation mark "
        .replace('\u{201D}', "\"") // Right double quotation mark "
        .replace('\u{2018}', "'") // Left single quotation mark '
        .replace('\u{2019}', "'") // Right single quotation mark '
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

/// Split text into sentences while preserving original punctuation.
/// Returns a vector of sentences including their terminating punctuation.
fn split_sentences_preserve_punct(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut last_end = 0;

    // Find each sentence boundary (punctuation + whitespace)
    for m in SENTENCE_BOUNDARY_RE.find_iter(text) {
        // Include everything up to and including the punctuation (but not trailing whitespace)
        let boundary_start = m.start();
        let boundary_text = m.as_str();

        // Find where the punctuation ends (before whitespace)
        let punct_end = boundary_text
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| boundary_start + i)
            .unwrap_or(m.end());

        let sentence = text[last_end..punct_end].trim();
        if !sentence.is_empty() {
            sentences.push(sentence.to_string());
        }
        last_end = m.end();
    }

    // Don't forget the final segment (text after last sentence boundary)
    let tail = text[last_end..].trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }

    sentences
}

/// Internal implementation of chunk_text (pure Rust, no PyO3 dependencies).
/// Returns None if target_size is 0, otherwise returns the chunks.
///
/// Uses character counts (Unicode code points) for sizing, not bytes.
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

    // Split into sentences, preserving original punctuation
    let sentences = split_sentences_preserve_punct(text);

    if sentences.is_empty() {
        // No sentence boundaries found, return as single chunk or split by size
        if char_len(text) <= target_size {
            return Some(vec![text.to_string()]);
        }
        // Fall back to simple character-based splitting for very long text without periods
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
    let mut current_chunk_chars: usize = 0;

    // Use VecDeque for O(1) pop_front instead of Vec::remove(0) which is O(n)
    let mut overlap_buffer: VecDeque<String> = VecDeque::new();
    let mut overlap_len_chars: usize = 0;

    for sentence in sentences {
        let sentence_chars = char_len(&sentence);
        // Add a space separator if not the first sentence in the chunk
        let separator = if current_chunk.is_empty() { "" } else { " " };
        let separator_chars = separator.len(); // Always 0 or 1 for ASCII space

        // Check if adding this sentence would exceed target
        if !current_chunk.is_empty()
            && current_chunk_chars + separator_chars + sentence_chars > target_size
        {
            // Save current chunk
            chunks.push(current_chunk.trim().to_string());

            // Start new chunk with overlap from previous sentences
            current_chunk = overlap_buffer
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            current_chunk_chars = char_len(&current_chunk);

            // Don't clear overlap_buffer - we keep it for continuity
        }

        // Add sentence to current chunk
        if !current_chunk.is_empty() {
            current_chunk.push(' ');
            current_chunk_chars += 1;
        }
        current_chunk.push_str(&sentence);
        current_chunk_chars += sentence_chars;

        // Track recent sentences for overlap (using character count)
        overlap_buffer.push_back(sentence.clone());
        overlap_len_chars += sentence_chars + 1; // +1 for space separator

        // Trim overlap buffer to stay within overlap limit (fix: recompute in loop!)
        while overlap_len_chars > overlap && overlap_buffer.len() > 1 {
            if let Some(removed) = overlap_buffer.pop_front() {
                overlap_len_chars = overlap_len_chars.saturating_sub(char_len(&removed) + 1);
            }
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
/// - Potential ticker symbols (sorted alphabetically for deterministic output)
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

    // Extract potential ticker symbols (filter common words, dedupe, sort for determinism)
    let mut tickers: Vec<String> = TICKER_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .filter(|t| !COMMON_TICKER_STOPWORDS.contains(t.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    tickers.sort(); // Deterministic ordering for reproducible output
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
        dict.set_item("char_count", char_len(chunk))?; // True character count, not bytes

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
    fn test_clean_text_nfkc_normalization() {
        // Test that NFKC normalization is actually applied
        // The "ﬁ" ligature (U+FB01) should become "fi"
        let input = "ﬁnance";
        let result = clean_text(input);
        assert_eq!(result, "finance");
    }

    #[test]
    fn test_chunk_text() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence.";
        let chunks = chunk_text_impl(text, 40, 10).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_zero_size_returns_none() {
        let text = "Some text.";
        let result = chunk_text_impl(text, 0, 10);
        assert!(result.is_none());
    }

    #[test]
    fn test_chunk_text_preserves_punctuation() {
        // Test that original punctuation is preserved
        let text = "What is this? It is great! Really.";
        let chunks = chunk_text_impl(text, 100, 10).unwrap();
        assert_eq!(chunks.len(), 1);
        // Should preserve the ? and ! instead of converting to .
        assert!(chunks[0].contains("?"));
        assert!(chunks[0].contains("!"));
    }

    #[test]
    fn test_chunk_text_overlap_correctness() {
        // Test that overlap actually works and is measured in characters
        let text = "One. Two. Three. Four. Five.";
        let chunks = chunk_text_impl(text, 15, 5).unwrap();
        // With such small chunks, we should get multiple chunks with overlap
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_standardize_quotes() {
        // Input with curly quotes (Unicode)
        let input = "\u{201C}Hello\u{201D} and \u{2018}world\u{2019}";
        let result = clean_text(input);
        assert_eq!(result, "\"Hello\" and 'world'");
    }

    #[test]
    fn test_char_len_vs_byte_len() {
        // Emoji is multiple bytes but one character (actually one grapheme cluster)
        let emoji = "😀";
        assert_eq!(char_len(emoji), 1);
        assert_eq!(emoji.len(), 4); // UTF-8 bytes

        // Accented character
        let accented = "é";
        assert_eq!(char_len(accented), 1);
    }

    #[test]
    fn test_split_sentences_preserve_punct() {
        let text = "Hello! How are you? I am fine.";
        let sentences = split_sentences_preserve_punct(text);
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0], "Hello!");
        assert_eq!(sentences[1], "How are you?");
        assert_eq!(sentences[2], "I am fine.");
    }
}
