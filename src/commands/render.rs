//! CLI output formatting (SPEC §13).
//!
//! The output identifies the returned range and prefixes each line with its
//! line number. The line-number presentation is metadata: it is added to the
//! returned representation and MUST NOT alter the underlying file content.

/// Render a read result for stdout.
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
    for (i, line_text) in text.split('\n').enumerate() {
        // split('\n') yields a trailing empty element for a final "\n";
        // skip it so the number of rendered lines matches the range.
        if i == text.split('\n').count() - 1 && line_text.is_empty() {
            continue;
        }
        let line_no = result.start_line + i;
        out.push_str(&format!("{line_no:>width$} | {line_text}\n"));
    }

    out
}
