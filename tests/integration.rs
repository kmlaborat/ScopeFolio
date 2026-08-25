use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Path to the scopefolio binary (built in debug mode).
fn binary_path() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("scopefolio")
}

/// Creates a temporary file with the given content.
fn create_temp_file(content: &[u8]) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let file_path = temp_dir.path().join("test_file.txt");
    std::fs::write(&file_path, content).expect("failed to write temp file");
    (temp_dir, file_path)
}

/// Runs the scopefolio binary with the given arguments.
fn run_scopefolio(args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args(args)
        .output()
        .expect("failed to execute scopefolio")
}

/// Builds file content with numbered lines: "L1\nL2\n...\nL{count}\n".
fn numbered_lines(count: usize) -> String {
    (1..=count).map(|i| format!("L{i}\n")).collect()
}

// ─── Partitioning (canonical v0.2.0 cases, default t=400) ──────

#[test]
fn small_file_returns_whole_file() {
    let (_dir, path) = create_temp_file(b"L1\nL2\nL3\n");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "2"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:1-3", path.to_str().unwrap())));
    assert!(stdout.contains("| L1"));
    assert!(stdout.contains("| L3"));
}

#[test]
fn canonical_n453_single_leaf() {
    // n=453, t=400: k = round(453/400) = 1 → single leaf [1, 453].
    let content = numbered_lines(453);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for line in ["1", "226", "227", "453"] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("{}:1-453", path.to_str().unwrap())),
            "line {line}: got {:?}",
            stdout.lines().next().unwrap()
        );
    }
}

#[test]
fn canonical_n600_tie_two_equal_leaves() {
    // n=600, t=400: k = round(1.5) = 2 (tie up) → leaves [1,300],
    // [301,600].
    let content = numbered_lines(600);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for (line, expected) in [("1", "1-300"), ("300", "1-300"), ("301", "301-600"), ("600", "301-600")] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("{}:{}", path.to_str().unwrap(), expected)),
            "line {line}: got {:?}",
            stdout.lines().next().unwrap()
        );
    }
}

#[test]
fn canonical_n800_two_equal_leaves() {
    // n=800, t=400: k = 2 → leaves [1,400], [401,800].
    let content = numbered_lines(800);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "400"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:1-400", path.to_str().unwrap())));

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "401"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:401-800", path.to_str().unwrap())));
}

#[test]
fn canonical_n1000_three_leaves() {
    // n=1000, t=400: k = 3 → leaves [1,333], [334,666], [667,1000].
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for (line, expected) in [
        ("1", "1-333"),
        ("333", "1-333"),
        ("334", "334-666"),
        ("666", "334-666"),
        ("667", "667-1000"),
        ("1000", "667-1000"),
    ] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("{}:{}", path.to_str().unwrap(), expected)),
            "line {line}: got {:?}",
            stdout.lines().next().unwrap()
        );
    }
}

#[test]
fn canonical_n1200_three_equal_leaves() {
    // n=1200, t=400: k = 3 → leaves [1,400], [401,800], [801,1200].
    let content = numbered_lines(1200);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for (line, expected) in [
        ("1", "1-400"),
        ("400", "1-400"),
        ("401", "401-800"),
        ("800", "401-800"),
        ("801", "801-1200"),
        ("1200", "801-1200"),
    ] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("{}:{}", path.to_str().unwrap(), expected)),
            "line {line}: got {:?}",
            stdout.lines().next().unwrap()
        );
    }
}

#[test]
fn odd_line_count_small_file_single_leaf() {
    // n=101 < 3t/2 (t=400) → single leaf; the old uneven-final-partition
    // behavior is gone — the whole file is one partition.
    let content = numbered_lines(101);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for line in ["1", "101"] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.starts_with(&format!("{}:1-101", path.to_str().unwrap())));
    }
}

#[test]
fn large_file() {
    let content = numbered_lines(5000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "2500"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Target line must be contained in the header range.
    let header = stdout
        .lines()
        .next()
        .unwrap()
        .split(':')
        .next_back()
        .unwrap();
    let mut parts = header.split('-');
    let start: usize = parts.next().unwrap().parse().unwrap();
    let end: usize = parts.next().unwrap().parse().unwrap();
    assert!(start <= 2500 && 2500 <= end);
}

// ─── Line resolution ───────────────────────────────────────────

#[test]
fn first_line() {
    let content = numbered_lines(800);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "1"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:1-400", path.to_str().unwrap())));
}

#[test]
fn last_line() {
    let content = numbered_lines(800);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "800"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:401-800", path.to_str().unwrap())));
}

#[test]
fn line_immediately_before_and_after_boundary() {
    let content = numbered_lines(800);
    let (_dir, path) = create_temp_file(content.as_bytes());

    for (line, expected) in [
        ("1", "1-400"),
        ("399", "1-400"),
        ("400", "1-400"),
        ("401", "401-800"),
        ("402", "401-800"),
        ("800", "401-800"),
    ] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("{}:{}", path.to_str().unwrap(), expected)),
            "line {line}: got header {:?}",
            stdout.lines().next().unwrap()
        );
    }
}

#[test]
fn middle_of_partition() {
    let content = numbered_lines(800);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "200"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:1-400", path.to_str().unwrap())));
    assert!(stdout.contains("200 | L200"));
}

// ─── Offset (o = floor(r · t), default t=400) ──────────────────

#[test]
fn zero_offset_is_default() {
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "500"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Leaf [334, 666], no expansion.
    assert!(stdout.starts_with(&format!("{}:334-666", path.to_str().unwrap())));
}

#[test]
fn positive_offset_expands() {
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    // offset floor(0.1 · 400) = 40 → leaf [334,666] → [294, 706].
    let output = run_scopefolio(&[
        "read",
        "--file",
        path.to_str().unwrap(),
        "--line",
        "500",
        "--offset-ratio",
        "0.1",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:294-706", path.to_str().unwrap())));
}

#[test]
fn offset_at_file_beginning_clamps() {
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&[
        "read",
        "--file",
        path.to_str().unwrap(),
        "--line",
        "1",
        "--offset-ratio",
        "1.0",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Leaf [1,333], offset 400 → start clamped to 1, end 733.
    assert!(stdout.starts_with(&format!("{}:1-733", path.to_str().unwrap())));
}

#[test]
fn offset_at_file_end_clamps() {
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&[
        "read",
        "--file",
        path.to_str().unwrap(),
        "--line",
        "1000",
        "--offset-ratio",
        "1.0",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Leaf [667,1000], offset 400 → start 267, end clamped to 1000.
    assert!(stdout.starts_with(&format!("{}:267-1000", path.to_str().unwrap())));
}

#[test]
fn large_offset_clamps_to_whole_file() {
    let content = numbered_lines(100);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&[
        "read",
        "--file",
        path.to_str().unwrap(),
        "--line",
        "50",
        "--offset-ratio",
        "10.0",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:1-100", path.to_str().unwrap())));
}

// ─── Partition size configuration ───────────────────────────────

#[test]
fn custom_partition_lines() {
    let content = numbered_lines(100);
    let (_dir, path) = create_temp_file(content.as_bytes());

    // t=25: k = round(100/25) = 4 → leaves of exactly 25 lines.
    let output = run_scopefolio(&[
        "read",
        "--file",
        path.to_str().unwrap(),
        "--line",
        "60",
        "--partition-lines",
        "25",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("{}:51-75", path.to_str().unwrap())));
}

// ─── Determinism ────────────────────────────────────────────────

#[test]
fn repeated_reads_are_identical() {
    let content = numbered_lines(1000);
    let (_dir, path) = create_temp_file(content.as_bytes());

    let mut outputs = Vec::new();
    for line in ["1", "42", "500", "999", "1000"] {
        let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
        assert!(output.status.success());
        for _ in 0..2 {
            let repeated =
                run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", line]);
            assert_eq!(
                output.stdout, repeated.stdout,
                "non-deterministic output for line {line}"
            );
        }
        outputs.push(String::from_utf8(output.stdout).unwrap());
    }
    assert_eq!(outputs.len(), 5);
}

// ─── Content preservation ───────────────────────────────────────

#[test]
fn content_preserves_whitespace_and_line_numbers() {
    let content = "alpha\n    indented\n\ttabbed\nplain\n";
    let (_dir, path) = create_temp_file(content.as_bytes());

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "2"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("1 | alpha"));
    assert!(stdout.contains("2 |     indented"));
    assert!(stdout.contains("3 | \ttabbed"));
    assert!(stdout.contains("4 | plain"));
}

#[test]
fn crlf_content_is_preserved_not_normalized() {
    let content = b"alpha\r\nbeta\r\n";
    let (_dir, path) = create_temp_file(content);

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "1"]);
    assert!(output.status.success());
    // The raw stdout bytes must still contain \r from the source file.
    assert!(output.stdout.windows(2).any(|w| w == b"\r\n".as_slice()));
}

// ─── Error handling ─────────────────────────────────────────────

#[test]
fn file_not_found_is_deterministic_error() {
    let output = run_scopefolio(&["read", "--file", "/nonexistent/nope.txt", "--line", "1"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("FILE_NOT_FOUND"), "stderr: {stderr}");
}

#[test]
fn invalid_line_below_range() {
    let (_dir, path) = create_temp_file(b"L1\nL2\n");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "0"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("INVALID_LINE"), "stderr: {stderr}");
}

#[test]
fn invalid_line_above_range() {
    let (_dir, path) = create_temp_file(b"L1\nL2\n");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "3"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("INVALID_LINE"), "stderr: {stderr}");
}

#[test]
fn empty_file_is_invalid_line() {
    let (_dir, path) = create_temp_file(b"");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "1"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("INVALID_LINE"), "stderr: {stderr}");
}

#[test]
fn invalid_utf8_is_deterministic_error() {
    let (_dir, path) = create_temp_file(b"\xff\xfe\x00binary\n");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "1"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid UTF-8"), "stderr: {stderr}");
}

#[test]
fn no_silent_substitution_on_error() {
    // Requesting a line beyond EOF must not return a clamped range silently.
    let (_dir, path) = create_temp_file(b"L1\nL2\n");

    let output = run_scopefolio(&["read", "--file", path.to_str().unwrap(), "--line", "999"]);
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
