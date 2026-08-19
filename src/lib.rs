//! ScopeFolio — Deterministic scoped file reading.
//!
//! Given the current file and a line number, shows the appropriate local
//! scope (SPEC §12). The agent specifies WHERE; ScopeFolio determines
//! HOW MUCH (SPEC §14).
//!
//! # Library API
//!
//! ```no_run
//! use scopefolio::{read, ReadResult, ScopeFolioError};
//!
//! let result: ReadResult = read("SPEC.md", 597, 50, 0.0)?;
//! println!("range {}-{}", result.start_line, result.end_line);
//! println!("{}", String::from_utf8_lossy(&result.content));
//! # Ok::<(), ScopeFolioError>(())
//! ```

mod error;
pub(crate) mod lines;
pub(crate) mod partition;

use std::fmt;
use std::fs;

// ── Public constants ──────────────────────────────────────────

/// Default target number of lines per leaf partition (SPEC §7, §19).
pub const DEFAULT_PARTITION_LINES: usize = 50;

/// Default contextual offset ratio (SPEC §8, §19).
pub const DEFAULT_OFFSET_RATIO: f64 = 0.0;

// ── Public types ──────────────────────────────────────────────

/// Result of a successful read operation.
#[derive(Debug)]
pub struct ReadResult {
    /// First line of the returned range (1-based, inclusive).
    pub start_line: usize,
    /// Last line of the returned range (1-based, inclusive).
    pub end_line: usize,
    /// Raw bytes of the selected lines, exactly as they appear in the file.
    /// Line endings, whitespace and encoding are preserved (SPEC §13).
    pub content: Vec<u8>,
}

/// Errors that can occur during a read operation (SPEC §20).
#[derive(Debug)]
pub enum ScopeFolioError {
    /// The target file does not exist.
    FileNotFound,
    /// The requested line is outside the valid file range (SPEC §10).
    InvalidLine,
    /// `partition_lines` is not a positive integer.
    InvalidPartitionLines,
    /// `offset_ratio` is not a finite value >= 0.
    InvalidOffsetRatio,
    /// I/O or content error with a deterministic description.
    IoError(String),
}

impl fmt::Display for ScopeFolioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeFolioError::FileNotFound => write!(f, "{}", error::FILE_NOT_FOUND),
            ScopeFolioError::InvalidLine => write!(f, "INVALID_LINE"),
            ScopeFolioError::InvalidPartitionLines => {
                write!(f, "INVALID_PARTITION_LINES")
            }
            ScopeFolioError::InvalidOffsetRatio => write!(f, "INVALID_OFFSET_RATIO"),
            ScopeFolioError::IoError(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ScopeFolioError {}

// ── Public API ────────────────────────────────────────────────

/// Read the local scope around `line` from `file_path`.
///
/// Constructs the binary partition tree for the current file (SPEC §9),
/// locates the leaf partition containing `line`, expands it by
/// `offset_ratio * partition_lines` lines on each side (SPEC §8), clamps the
/// result to the file boundaries (SPEC §10), and returns the selected lines
/// byte-for-byte.
///
/// This operation is stateless and deterministic: the same file contents,
/// target line and configuration always produce the same range and content
/// (SPEC §18).
pub fn read(
    file_path: &str,
    line: usize,
    partition_lines: usize,
    offset_ratio: f64,
) -> Result<ReadResult, ScopeFolioError> {
    // Validate configuration (SPEC §20).
    if partition_lines == 0 {
        return Err(ScopeFolioError::InvalidPartitionLines);
    }
    if !offset_ratio.is_finite() || offset_ratio < 0.0 {
        return Err(ScopeFolioError::InvalidOffsetRatio);
    }

    // 1. Open the target file.
    let raw = fs::read(file_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ScopeFolioError::FileNotFound
        } else {
            ScopeFolioError::IoError(error::io_error(error::map_io_error_read(&e)))
        }
    })?;

    // 2. Determine its line structure (text content only; content is
    //    returned byte-for-byte with no encoding modification).
    if std::str::from_utf8(&raw).is_err() {
        return Err(ScopeFolioError::IoError(error::io_error(
            error::IoErrorKind::InvalidUtf8,
        )));
    }
    let line_map = lines::build(&raw);
    if line < 1 || line > line_map.line_count {
        return Err(ScopeFolioError::InvalidLine);
    }

    // 3. Construct the binary partition tree.
    let tree = partition::build_tree(line_map.line_count, partition_lines);

    // 4. Find the leaf partition containing `line`.
    let leaf = partition::find_leaf(&tree, line);

    // 5. Expand the partition according to `offset_ratio`.
    //    Offset is approximately `offset_ratio` times the target width
    //    (SPEC §8). Deterministic: IEEE-754 round-half-to-even.
    let offset = (partition_lines as f64 * offset_ratio).round() as usize;

    // 6. Clamp the resulting range to the file.
    let start_line = leaf.start.saturating_sub(offset).max(1);
    let end_line = (leaf.end + offset).min(line_map.line_count);

    // 7. Return the selected lines, byte-for-byte (SPEC §13).
    let (byte_start, _) = line_map.byte_range(start_line);
    let (_, byte_end) = line_map.byte_range(end_line);
    let content = raw[byte_start..byte_end].to_vec();

    Ok(ReadResult {
        start_line,
        end_line,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write content to a temp file and return (dir, path).
    fn temp_file(content: impl AsRef<[u8]>) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn read_basic() {
        let content: String = (1..=200).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 59, 50, 0.0).unwrap();
        // Line 59 → leaf [51, 100] (first split at 50).
        assert_eq!(result.start_line, 51);
        assert_eq!(result.end_line, 100);
        let expected: String = (51..=100).map(|i| format!("{i}\n")).collect();
        assert_eq!(result.content, expected.as_bytes());
    }

    #[test]
    fn read_first_leaf() {
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 1, 50, 0.0).unwrap();
        assert_eq!(result.start_line, 1);
        assert_eq!(result.end_line, 50);
        assert!(result.content.starts_with(b"1\n"));
        assert!(result.content.ends_with(b"50\n"));
    }

    #[test]
    fn read_last_line_clamps_upper_boundary() {
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 100, 50, 0.5).unwrap();
        // Leaf [51, 100], offset 25 → end clamped to 100.
        assert_eq!(result.end_line, 100);
    }

    #[test]
    fn read_first_line_clamps_lower_boundary() {
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 1, 50, 0.5).unwrap();
        // Leaf [1, 50], offset 25 → start clamped to 1.
        assert_eq!(result.start_line, 1);
    }

    #[test]
    fn offset_expands_both_sides() {
        let content: String = (1..=200).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        // Target 50, ratio 0.1 → offset 5 → leaf [51,100] → [46, 105].
        let result = read(path.to_str().unwrap(), 59, 50, 0.1).unwrap();
        assert_eq!(result.start_line, 46);
        assert_eq!(result.end_line, 105);
    }

    #[test]
    fn target_line_is_always_in_range() {
        let content: String = (1..=1193).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        for line in 1..=1193 {
            let result = read(path.to_str().unwrap(), line, 50, 0.1).unwrap();
            assert!(
                result.start_line <= line && line <= result.end_line,
                "line {line} not in {}-{}",
                result.start_line,
                result.end_line
            );
        }
    }

    #[test]
    fn small_file_single_leaf() {
        let (_dir, path) = temp_file(b"A\nB\nC\n");
        let result = read(path.to_str().unwrap(), 2, 50, 0.0).unwrap();
        assert_eq!((result.start_line, result.end_line), (1, 3));
        assert_eq!(result.content, b"A\nB\nC\n");
    }

    #[test]
    fn final_line_without_terminator_preserved() {
        let (_dir, path) = temp_file(b"A\nB\nC");
        let result = read(path.to_str().unwrap(), 3, 50, 0.0).unwrap();
        assert_eq!(result.content, b"A\nB\nC");
    }

    #[test]
    fn crlf_preserved() {
        let (_dir, path) = temp_file(b"A\r\nB\r\nC\r\n");
        let result = read(path.to_str().unwrap(), 2, 50, 0.0).unwrap();
        assert_eq!(result.content, b"A\r\nB\r\nC\r\n");
    }

    #[test]
    fn invalid_line_below_range() {
        let (_dir, path) = temp_file(b"A\nB\n");
        let err = read(path.to_str().unwrap(), 0, 50, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidLine));
    }

    #[test]
    fn invalid_line_above_range() {
        let (_dir, path) = temp_file(b"A\nB\n");
        let err = read(path.to_str().unwrap(), 3, 50, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidLine));
    }

    #[test]
    fn empty_file_is_invalid_line() {
        let (_dir, path) = temp_file(b"");
        let err = read(path.to_str().unwrap(), 1, 50, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidLine));
    }

    #[test]
    fn invalid_partition_lines() {
        let (_dir, path) = temp_file(b"A\n");
        let err = read(path.to_str().unwrap(), 1, 0, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidPartitionLines));
    }

    #[test]
    fn invalid_offset_ratio() {
        let (_dir, path) = temp_file(b"A\n");
        let err = read(path.to_str().unwrap(), 1, 50, -0.1).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidOffsetRatio));
        let err = read(path.to_str().unwrap(), 1, 50, f64::NAN).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidOffsetRatio));
    }

    #[test]
    fn file_not_found() {
        let err = read("no/such/file.txt", 1, 50, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::FileNotFound));
    }

    #[test]
    fn determinism_same_input_same_output() {
        let content: String = (1..=500).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);
        let path = path.to_str().unwrap();

        for line in [1, 25, 50, 51, 250, 499, 500] {
            let a = read(path, line, 50, 0.1).unwrap();
            let b = read(path, line, 50, 0.1).unwrap();
            assert_eq!(a.start_line, b.start_line);
            assert_eq!(a.end_line, b.end_line);
            assert_eq!(a.content, b.content);
        }
    }

    #[test]
    fn content_preservation_matches_source_lines() {
        let source = "line one\n  indented\nline three\n";
        let (_dir, path) = temp_file(source.as_bytes());

        let result = read(path.to_str().unwrap(), 2, 50, 0.0).unwrap();
        assert_eq!(result.content, source.as_bytes());
    }

    #[test]
    fn custom_partition_lines() {
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        // Target width 25: root splits [1,50]/[51,100], then into 25-line leaves.
        let result = read(path.to_str().unwrap(), 60, 25, 0.0).unwrap();
        assert_eq!((result.start_line, result.end_line), (51, 75));
    }

    #[test]
    fn error_display_codes() {
        assert_eq!(
            format!("{}", ScopeFolioError::FileNotFound),
            "FILE_NOT_FOUND"
        );
        assert_eq!(format!("{}", ScopeFolioError::InvalidLine), "INVALID_LINE");
        assert_eq!(
            format!("{}", ScopeFolioError::InvalidPartitionLines),
            "INVALID_PARTITION_LINES"
        );
        assert_eq!(
            format!("{}", ScopeFolioError::InvalidOffsetRatio),
            "INVALID_OFFSET_RATIO"
        );
        assert_eq!(
            format!(
                "{}",
                ScopeFolioError::IoError("IO_ERROR: read failure".to_string())
            ),
            "IO_ERROR: read failure"
        );
    }
}
