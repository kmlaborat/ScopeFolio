use super::render;

/// Execute the read command.
/// Returns 0 on success, 1 on error.
pub fn execute(file: &str, line: usize, partition_lines: usize, offset_ratio: f64) -> i32 {
    match scopefolio::read(file, line, partition_lines, offset_ratio) {
        Err(e) => {
            eprintln!("{}", e);
            1
        }
        Ok(result) => {
            print!("{}", render::render(file, &result));
            0
        }
    }
}
