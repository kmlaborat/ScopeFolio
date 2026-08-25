//! CLI output formatting (SPEC_v0.2.0 §14).
//!
//! The output identifies the returned range and prefixes each line with its
//! line number. The line-number presentation is metadata: it is added to the
//! returned representation and MUST NOT alter the underlying file content.

/// Render a read result for stdout.
///
/// Presentation vs. content (SPEC §14): the displayed line text is the
/// source line's bytes verbatim. In CRLF files each displayed line
/// therefore carries a trailing `"\r"`; the display deliberately does
/// NOT strip it, so terminal output may look odd but the rendered text
/// stays byte-faithful to the file. The source bytes themselves are
/// never modified (content preservation, SPEC §14).
pub fn render(file: &str, result: &scopefolio::ReadResult) -> String {
    let text = String::from_utf8_lossy(&result.content);
    let mut out = String::new();

    // Range header.
    out.push_str(file);
    out.push(':');
    out.push_str(&result.start_line.to_string());
    out.push('-');
    out.push_str(&result.end_line.to_string());
    out.push('\n');
    out.push('\n');

    // Numbered lines. Prefix width is uniform across the range.
    let width = result.end_line.to_string().len();
    // `split('\n')` yields one trailing empty element iff the text ends
    // with "\n"; compute that once (O(L) total, not O(L^2)) and skip
    // only that tail element so the rendered line count matches the
    // range.
    let trailing_newline = text.ends_with('\n');
    let element_count = text.split('\n').count();
    for (i, line_text) in text.split('\n').enumerate() {
        if trailing_newline && i + 1 == element_count {
            continue; // final "\n" produced an empty tail element
        }
        let line_no = result.start_line + i;
        out.push_str(&format!("{line_no:>width$} | {line_text}\n"));
    }

    out
}
