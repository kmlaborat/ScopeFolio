//! ScopeFolio — Deterministic scoped file reading.
//!
//! Given the current file and a line number, shows the appropriate local
//! scope (SPEC_v0.2.0 §11). The agent specifies WHERE; ScopeFolio
//! determines HOW MUCH (SPEC_v0.2.0 §8, §10).
//!
//! # Library API
//!
//! ```no_run
//! use scopefolio::{read, ReadResult, ScopeFolioError};
//!
//! let result: ReadResult = read("SPEC_v0.2.0.md", 296, 400, 0.0)?;
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

/// Default target number of lines per leaf partition (SPEC_v0.2.0 §7, §8).
///
/// This is a *target leaf size*, not a maximum width: an even split of
/// `n` lines into `k = max(1, round(n / t))` leaves makes each leaf
/// `floor(n/k)` or `ceil(n/k)` lines, inside `[3t/4, 3t/2)` when `k > 1`
/// (§8.3).
pub const DEFAULT_PARTITION_LINES: usize = 400;

/// Default contextual offset ratio (SPEC_v0.2.0 §10, §17).
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
    /// Line endings, whitespace and encoding are preserved (SPEC_v0.2.0 §14).
    pub content: Vec<u8>,
}

/// Errors that can occur during a read operation (SPEC_v0.2.0 §18).
#[derive(Debug)]
pub enum ScopeFolioError {
    /// The target file does not exist.
    FileNotFound,
    /// The requested line is outside the valid file range (SPEC_v0.2.0 §11).
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
/// Constructs the canonical even-split partition for the current file
/// (SPEC_v0.2.0 §8, §9), locates the leaf partition containing `line`
/// (§11), expands it by `offset = floor(offset_ratio · partition_lines)`
/// lines on each side (§10), clamps the result to the file boundaries, and
/// returns the selected lines byte-for-byte.
///
/// This operation is stateless and deterministic: the same file contents,
/// target line and configuration always produce the same range and content
/// (SPEC_v0.2.0 §12).
pub fn read(
    file_path: &str,
    line: usize,
    partition_lines: usize,
    offset_ratio: f64,
) -> Result<ReadResult, ScopeFolioError> {
    // Validate configuration (SPEC_v0.2.0 §17, §18).
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
    //    Offset is `floor(offset_ratio · partition_lines)` lines on each
    //    side (SPEC_v0.2.0 §10). Computed in IEEE-754 f64; the exact
    //    integer result follows for the standard ratios (e.g. 0.1·400 =
    //    40.0 exactly in f64, floored to 40).
    let offset = (partition_lines as f64 * offset_ratio).floor() as usize;

    // 6. Clamp the resulting range to the file.
    let start_line = leaf.start.saturating_sub(offset).max(1);
    let end_line = (leaf.end + offset).min(line_map.line_count);

    // 7. Return the selected lines, byte-for-byte (SPEC_v0.2.0 §14).
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
        // n=800, t=400: k=2 → leaves [1, 400], [401, 800].
        let content: String = (1..=800).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 100, 400, 0.0).unwrap();
        assert_eq!(result.start_line, 1);
        assert_eq!(result.end_line, 400);
        let expected: String = (1..=400).map(|i| format!("{i}\n")).collect();
        assert_eq!(result.content, expected.as_bytes());
    }

    #[test]
    fn read_first_leaf() {
        // n=1000, t=400: k=3 → leaves [1, 333], [334, 666], [667, 1000].
        let content: String = (1..=1000).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 1, 400, 0.0).unwrap();
        assert_eq!(result.start_line, 1);
        assert_eq!(result.end_line, 333);
        assert!(result.content.starts_with(b"1\n"));
        assert!(result.content.ends_with(b"333\n"));
    }

    #[test]
    fn read_last_line_clamps_upper_boundary() {
        let content: String = (1..=1000).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 1000, 400, 0.5).unwrap();
        // Leaf [667, 1000], offset floor(0.5·400) = 200 → end clamped to
        // 1000.
        assert_eq!(result.end_line, 1000);
    }

    #[test]
    fn read_first_line_clamps_lower_boundary() {
        let content: String = (1..=1000).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 1, 400, 0.5).unwrap();
        // Leaf [1, 333], offset floor(0.5·400) = 200 → start clamped to 1.
        assert_eq!(result.start_line, 1);
    }

    #[test]
    fn offset_expands_both_sides() {
        let content: String = (1..=1000).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        // Line 500 → leaf [334, 666]; offset floor(0.1·400) = 40 →
        // [294, 706].
        let result = read(path.to_str().unwrap(), 500, 400, 0.1).unwrap();
        assert_eq!(result.start_line, 294);
        assert_eq!(result.end_line, 706);
    }

    #[test]
    fn target_line_is_always_in_range() {
        // Canonical n=1200, t=400: k=3 → leaves [1, 400], [401, 800],
        // [801, 1200]. Every line must be inside its expanded range.
        let content: String = (1..=1200).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        for line in 1..=1200 {
            let result = read(path.to_str().unwrap(), line, 400, 0.1).unwrap();
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
        // n=3 < 3t/2 for any target → single leaf [1, 3].
        let (_dir, path) = temp_file(b"A\nB\nC\n");
        let result = read(path.to_str().unwrap(), 2, 400, 0.0).unwrap();
        assert_eq!((result.start_line, result.end_line), (1, 3));
        assert_eq!(result.content, b"A\nB\nC\n");
    }

    #[test]
    fn default_t_400_single_leaf() {
        // Canonical n=453 with the default target t=400: k=1 → the whole
        // file is one leaf, so any target line returns [1, 453].
        assert_eq!(DEFAULT_PARTITION_LINES, 400);
        let content: String = (1..=453).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        let result = read(path.to_str().unwrap(), 100, DEFAULT_PARTITION_LINES, 0.0).unwrap();
        assert_eq!((result.start_line, result.end_line), (1, 453));
    }

    #[test]
    fn final_line_without_terminator_preserved() {
        let (_dir, path) = temp_file(b"A\nB\nC");
        let result = read(path.to_str().unwrap(), 3, 400, 0.0).unwrap();
        assert_eq!(result.content, b"A\nB\nC");
    }

    #[test]
    fn crlf_preserved() {
        let (_dir, path) = temp_file(b"A\r\nB\r\nC\r\n");
        let result = read(path.to_str().unwrap(), 2, 400, 0.0).unwrap();
        assert_eq!(result.content, b"A\r\nB\r\nC\r\n");
    }

    #[test]
    fn invalid_line_below_range() {
        let (_dir, path) = temp_file(b"A\nB\n");
        let err = read(path.to_str().unwrap(), 0, 400, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidLine));
    }

    #[test]
    fn invalid_line_above_range() {
        let (_dir, path) = temp_file(b"A\nB\n");
        let err = read(path.to_str().unwrap(), 3, 400, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidLine));
    }

    #[test]
    fn empty_file_is_invalid_line() {
        let (_dir, path) = temp_file(b"");
        let err = read(path.to_str().unwrap(), 1, 400, 0.0).unwrap_err();
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
        let err = read(path.to_str().unwrap(), 1, 400, -0.1).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidOffsetRatio));
        let err = read(path.to_str().unwrap(), 1, 400, f64::NAN).unwrap_err();
        assert!(matches!(err, ScopeFolioError::InvalidOffsetRatio));
    }

    #[test]
    fn file_not_found() {
        let err = read("no/such/file.txt", 1, 400, 0.0).unwrap_err();
        assert!(matches!(err, ScopeFolioError::FileNotFound));
    }

    #[test]
    fn determinism_same_input_same_output() {
        let content: String = (1..=5000).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);
        let path = path.to_str().unwrap();

        for line in [1, 25, 50, 51, 250, 499, 500, 2500, 5000] {
            let a = read(path, line, 400, 0.1).unwrap();
            let b = read(path, line, 400, 0.1).unwrap();
            assert_eq!(a.start_line, b.start_line);
            assert_eq!(a.end_line, b.end_line);
            assert_eq!(a.content, b.content);
        }
    }

    #[test]
    fn content_preservation_matches_source_lines() {
        let source = "line one\n  indented\nline three\n";
        let (_dir, path) = temp_file(source.as_bytes());

        let result = read(path.to_str().unwrap(), 2, 400, 0.0).unwrap();
        assert_eq!(result.content, source.as_bytes());
    }

    #[test]
    fn custom_partition_lines() {
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let (_dir, path) = temp_file(&content);

        // Target 25: even split into k = round(100/25) = 4 leaves of
        // exactly 25 lines each → line 60 → [51, 75].
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
