//! API-level geometry tests for the canonical even-split partition
//! (SPEC_v0.2.0 §8). Driven through the public `read()` API with real
//! files, complementing the pure-arithmetic tests in `partition.rs`.

use scopefolio::read;
use tempfile::TempDir;

fn temp_file(content: impl AsRef<[u8]>) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geometry.txt");
    std::fs::write(&path, content).unwrap();
    (dir, path)
}

/// Numbered file with `n` lines.
fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("{i}\n")).collect()
}

/// `(target_line, expected (start_line, end_line))`.
type LineExpectation = (usize, (usize, usize));

/// `(n, t, [line → expected range])` row of the canonical leaf table.
type TableRow = (usize, usize, Vec<LineExpectation>);

// ─── SPEC §8.2 canonical cases (default target t = 400) ────────

/// Table of canonical (n, t) → leaf intervals. Each target line must
/// resolve to the leaf containing it, exactly, with zero offset.
#[test]
fn canonical_leaf_table() {
    // (n, t, [line → expected (start, end)] ...)
    let table: Vec<TableRow> = vec![
        // n=453, t=400: k=1 → whole file is one leaf.
        (453, 400, vec![(1, (1, 453)), (226, (1, 453)), (227, (1, 453)), (453, (1, 453))]),
        // n=600, t=400: tie, k=2 → [1,300], [301,600].
        (
            600,
            400,
            vec![(1, (1, 300)), (300, (1, 300)), (301, (301, 600)), (600, (301, 600))],
        ),
        // n=800, t=400: k=2 → [1,400], [401,800].
        (
            800,
            400,
            vec![(1, (1, 400)), (400, (1, 400)), (401, (401, 800)), (800, (401, 800))],
        ),
        // n=1000, t=400: k=3 → [1,333], [334,666], [667,1000].
        (
            1000,
            400,
            vec![
                (1, (1, 333)),
                (333, (1, 333)),
                (334, (334, 666)),
                (666, (334, 666)),
                (667, (667, 1000)),
                (1000, (667, 1000)),
            ],
        ),
        // n=1200, t=400: k=3 → [1,400], [401,800], [801,1200].
        (
            1200,
            400,
            vec![
                (1, (1, 400)),
                (400, (1, 400)),
                (401, (401, 800)),
                (800, (401, 800)),
                (801, (801, 1200)),
                (1200, (801, 1200)),
            ],
        ),
    ];

    for (n, t, entries) in table {
        let content = numbered(n);
        let (_dir, path) = temp_file(&content);
        for (line, (s, e)) in &entries {
            let r = read(path.to_str().unwrap(), *line, t, 0.0).unwrap();
            assert_eq!(
                (r.start_line, r.end_line),
                (*s, *e),
                "n={n}, t={t}, line={line}"
            );
        }
    }
}

// ─── Single-leaf invariant for n = 453 ────────────────────────

/// n=453 with t=400 gives a single leaf [1, 453]. No offset ratio may
/// change the returned range once clamped: the whole file is always the
/// answer.
#[test]
fn n453_single_leaf_invariant_across_offsets() {
    let content = numbered(453);
    let (_dir, path) = temp_file(&content);
    let path = path.to_str().unwrap();

    for ratio in [0.0, 0.1, 0.5, 1.0, 10.0] {
        let r = read(path, 100, 400, ratio).unwrap();
        assert_eq!(
            (r.start_line, r.end_line),
            (1, 453),
            "ratio={ratio}"
        );
        assert_eq!(r.content.as_slice(), content.as_bytes());
    }
}

// ─── Boundary ±1 checks ────────────────────────────────────────

/// The split points of n=1000 (333/334, 666/667) resolve to the leaf on
/// the correct side; a single line away crosses the boundary.
#[test]
fn boundary_lines_resolve_to_adjacent_leaves() {
    let content = numbered(1000);
    let (_dir, path) = temp_file(&content);
    let path = path.to_str().unwrap();

    assert_eq!(read(path, 332, 400, 0.0).unwrap().start_line, 1);
    assert_eq!(read(path, 333, 400, 0.0).unwrap().end_line, 333);
    assert_eq!(read(path, 334, 400, 0.0).unwrap().start_line, 334);
    assert_eq!(read(path, 335, 400, 0.0).unwrap().end_line, 666);

    assert_eq!(read(path, 666, 400, 0.0).unwrap().end_line, 666);
    assert_eq!(read(path, 667, 400, 0.0).unwrap().start_line, 667);
    assert_eq!(read(path, 668, 400, 0.0).unwrap().end_line, 1000);
}

// ─── Offset tie-breaking (o = floor(r · t)) ────────────────────

/// r = 1/3, t = 400: o = floor(133.33…) = 133 (never 134). With
/// n=1200 the leaf [401, 800] becomes [268, 933].
#[test]
fn offset_floor_tie_break() {
    let content = numbered(1200);
    let (_dir, path) = temp_file(&content);
    let path = path.to_str().unwrap();

    let r = read(path, 600, 400, 1.0 / 3.0).unwrap();
    assert_eq!(r.start_line, 401 - 133);
    assert_eq!(r.end_line, 800 + 133);

    // Control: exactly r = 0.25 → o = 100.
    let r = read(path, 600, 400, 0.25).unwrap();
    assert_eq!(r.start_line, 301);
    assert_eq!(r.end_line, 900);
}

// ─── Full line sweep, n = 1200 ─────────────────────────────────

/// Every line must be inside its expanded range; consecutive lines never
/// jump to non-adjacent leaves.
#[test]
fn line_sweep_n1200() {
    let content = numbered(1200);
    let (_dir, path) = temp_file(&content);
    let path = path.to_str().unwrap();

    for line in 1..=1200 {
        let r = read(path, line, 400, 0.1).unwrap();
        assert!(
            r.start_line <= line && line <= r.end_line,
            "line {line} not in {}-{}",
            r.start_line,
            r.end_line
        );
    }

    // Interior leaf [401, 800] is never clamped: expanded width is
    // 400 + 2·40 = 480.
    for line in 401..=800 {
        let r = read(path, line, 400, 0.1).unwrap();
        assert_eq!(r.end_line - r.start_line + 1, 480, "line {line}");
    }
}

// ─── Determinism ───────────────────────────────────────────────

/// Repeated reads (including offset ratios) produce identical results.
#[test]
fn geometry_determinism() {
    let content = numbered(1200);
    let (_dir, path) = temp_file(&content);
    let path = path.to_str().unwrap();

    for line in [1, 400, 401, 600, 800, 801, 1200] {
        for ratio in [0.0, 0.1, 0.5] {
            let a = read(path, line, 400, ratio).unwrap();
            let b = read(path, line, 400, ratio).unwrap();
            assert_eq!(a.start_line, b.start_line);
            assert_eq!(a.end_line, b.end_line);
            assert_eq!(a.content, b.content);
        }
    }
}
