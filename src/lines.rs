//! Line structure derivation from raw file bytes (SPEC §9 step 1-2).
//!
//! A line is a sequence of bytes terminated by `\n`. The final line is
//! counted even when it has no trailing terminator. Line terminators are
//! included in the line's byte range so that selected lines can be returned
//! byte-for-byte as they appear in the file (SPEC §13: line endings MUST be
//! preserved — no CRLF normalization is performed).

/// Byte map of a file's line structure.
#[derive(Debug, Clone)]
pub struct LineMap {
    /// Total number of lines (0 for an empty file).
    pub line_count: usize,
    /// Inclusive byte start of each line (indexed from 0).
    pub starts: Vec<usize>,
    /// Exclusive byte end of each line (indexed from 0).
    pub ends: Vec<usize>,
}

impl LineMap {
    /// Line number (1-based) → inclusive byte range `[start, end)`.
    pub fn byte_range(&self, line: usize) -> (usize, usize) {
        let i = line - 1;
        (self.starts[i], self.ends[i])
    }
}

/// Derive the line structure from raw file bytes.
pub fn build(content: &[u8]) -> LineMap {
    let mut starts: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();

    let len = content.len();
    if len == 0 {
        return LineMap {
            line_count: 0,
            starts,
            ends,
        };
    }

    let mut line_start = 0;
    for (i, b) in content.iter().enumerate() {
        if *b == b'\n' {
            starts.push(line_start);
            ends.push(i + 1);
            line_start = i + 1;
        }
    }
    // Final line without trailing terminator.
    if line_start < len {
        starts.push(line_start);
        ends.push(len);
    }

    LineMap {
        line_count: starts.len(),
        starts,
        ends,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_has_zero_lines() {
        let map = build(b"");
        assert_eq!(map.line_count, 0);
    }

    #[test]
    fn simple_lf_lines() {
        let map = build(b"AAA\nBBB\nCCC\n");
        assert_eq!(map.line_count, 3);
        assert_eq!(map.byte_range(1), (0, 4));
        assert_eq!(map.byte_range(2), (4, 8));
        assert_eq!(map.byte_range(3), (8, 12));
    }

    #[test]
    fn final_line_without_terminator_is_counted() {
        let map = build(b"AAA\nBBB\nCCC");
        assert_eq!(map.line_count, 3);
        assert_eq!(map.byte_range(3), (8, 11));
    }

    #[test]
    fn crlf_bytes_are_preserved_in_range() {
        let content = b"AAA\r\nBBB\r\n";
        let map = build(content);
        assert_eq!(map.line_count, 2);
        let (s, e) = map.byte_range(1);
        assert_eq!(&content[s..e], b"AAA\r\n");
        let (s, e) = map.byte_range(2);
        assert_eq!(&content[s..e], b"BBB\r\n");
    }

    #[test]
    fn only_newline_is_one_empty_line() {
        let map = build(b"\n");
        assert_eq!(map.line_count, 1);
        assert_eq!(map.byte_range(1), (0, 1));
    }
}
