//! Canonical even-split partition geometry (SPEC_v0.2.0 §8, §9).
//!
//! All partition math is integer arithmetic (§19). The tree is an
//! internal structure; the canonical leaf partition is derived purely
//! from `(n, t)` via the arithmetic boundary function `b(i)`.

use std::rc::Rc;

/// Canonical partition node (internal; not exposed in the public API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartitionNode {
    /// Inclusive start line.
    pub start: usize,
    /// Inclusive end line.
    pub end: usize,
    /// `None` on leaves.
    pub children: Option<(Rc<PartitionNode>, Rc<PartitionNode>)>,
}

impl PartitionNode {
    /// Line interval as `[start, end]`.
    #[allow(dead_code)] // exercised by tests
    pub fn interval(&self) -> [usize; 2] {
        [self.start, self.end]
    }
}

// ─── Canonical even-split geometry (SPEC_v0.2.0 §8) ──────────────

/// Leaf count: `k = max(1, round_half_up(n / t))`.
///
/// Computed as `k = max(1, floor((2n + t) / (2t)))` in pure integer
/// arithmetic (§8.1, §19). `round(n/t)` with ties going up is
/// `floor(n/t) + (2·(n mod t) >= t)`.
fn leaf_count(n: usize, t: usize) -> usize {
    debug_assert!(t >= 1, "target size must be positive");
    let q = n / t;
    let rem = n % t;
    let k = q + if 2 * rem >= t { 1 } else { 0 };
    k.max(1)
}

/// Leaf boundary prefix: `b(0) = 0`, `b(i) = floor(n·i/k)` for
/// `1 <= i < k`, `b(k) = n` (§8.2, §19). `n·i` is computed in `u128`
/// so it cannot overflow for `n <= usize::MAX`.
fn boundaries(n: usize, k: usize) -> Vec<usize> {
    assert!(k >= 1);
    let mut b = vec![0usize; k + 1];
    for i in 1..k {
        b[i] = (n as u128 * i as u128 / k as u128) as usize;
    }
    b[k] = n;
    b
}

/// Canonical binary tree for the `(n, t)` pair (§9).
///
/// The tree is derived from the leaf-index range `[0, k-1]`: a node
/// with leaf-index range `[lo, hi]` covers line interval
/// `[b(lo)+1, b(hi+1)]` and splits at `m = floor((lo+hi)/2)`, with the
/// left child covering leaf-index range `[lo, m]` and the right child
/// `[m+1, hi]`. Leaves map to `[b(j)+1, b(j+1)]` (§8.2). Because the
/// boundary function is monotonic, every node's span is a contiguous
/// range of leaves, so a tree walk always resolves to the same leaf
/// index as the closed-form oracle `ceil(line·k/n) − 1`.
pub(crate) fn build_tree(n: usize, t: usize) -> Rc<PartitionNode> {
    debug_assert!(n >= 1, "line count must be positive");
    let k = leaf_count(n, t);
    let b = boundaries(n, k);
    Rc::new(build_node(0, k - 1, &b))
}

fn build_node(lo: usize, hi: usize, b: &[usize]) -> PartitionNode {
    assert!(lo <= hi, "empty leaf-index range");
    let start = b[lo] + 1;
    let end = b[hi + 1];
    if lo == hi {
        PartitionNode {
            start,
            end,
            children: None,
        }
    } else {
        let m = (lo + hi) / 2;
        PartitionNode {
            start,
            end,
            children: Some((
                Rc::new(build_node(lo, m, b)),
                Rc::new(build_node(m + 1, hi, b)),
            )),
        }
    }
}

/// Find the leaf containing the 1-based `line` (§11).
pub(crate) fn find_leaf<'a>(node: &'a Rc<PartitionNode>, line: usize) -> &'a PartitionNode {
    let mut cur = node;
    while let Some((l, r)) = &cur.children {
        cur = if line <= l.end { l } else { r };
    }
    cur
}

/// Closed-form leaf index oracle: `j = ceil(line·k/n) − 1` (SPEC_v0.2.0 §9).
///
/// `ceil` computed as `(line·k + n − 1) / n` in `u128` to avoid
/// overflow (§19).
#[allow(dead_code)] // closed-form oracle cross-checked by tests
pub(crate) fn leaf_index(n: usize, k: usize, line: usize) -> usize {
    assert!(1 <= line && line <= n);
    assert!(k >= 1);
    ((line as u128 * k as u128 + n as u128 - 1) / n as u128) as usize - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `b(i) = floor(n*i/k)` reference for `0 <= i <= k`.
    fn b(n: usize, k: usize, i: usize) -> usize {
        (n as u128 * i as u128 / k as u128) as usize
    }

    /// Leaf `j` interval `[b(j)+1, b(j+1)]` reference.
    fn leaf_interval(n: usize, k: usize, j: usize) -> [usize; 2] {
        [b(n, k, j) + 1, b(n, k, j + 1)]
    }

    fn children(node: &Rc<PartitionNode>) -> [&Rc<PartitionNode>; 2] {
        match &node.children {
            Some((l, r)) => [&l, &r],
            None => panic!("expected internal node"),
        }
    }

    // ─── Canonical test cases (SPEC_v0.2.0 §8.2, §20) ───────────

    #[test]
    fn canonical_n453_t400_single_leaf() {
        // n=453, t=400: k=1 → whole file is one leaf [1, 453].
        assert_eq!(leaf_count(453, 400), 1);
        let tree = build_tree(453, 400);
        assert_eq!(tree.interval(), [1, 453]);
        assert!(tree.children.is_none());
    }

    #[test]
    fn canonical_n599_t400_single_leaf() {
        // n=599, t=400: 599 < 3t/2 = 600 → k=1, the whole file.
        // (SPEC_v0.2.0 §8.2 discriminating case.)
        assert_eq!(leaf_count(599, 400), 1);
        let tree = build_tree(599, 400);
        assert_eq!(tree.interval(), [1, 599]);
        assert!(tree.children.is_none());
    }

    #[test]
    fn canonical_n453_t200_unequal_split() {
        // n=453, t=200: k = round(453/200) = 2 → b(1) = floor(453/2)
        // = 226 → leaves [1, 226] (226 lines) and [227, 453] (227
        // lines). (SPEC_v0.2.0 §20 discriminating case: 226+227.)
        assert_eq!(leaf_count(453, 200), 2);
        let tree = build_tree(453, 200);
        let [l, r] = children(&tree);
        assert_eq!(l.interval(), [1, 226]);
        assert_eq!(r.interval(), [227, 453]);
        assert_eq!(find_leaf(&tree, 226).interval(), [1, 226]);
        assert_eq!(find_leaf(&tree, 227).interval(), [227, 453]);
    }

    #[test]
    fn canonical_n600_t400_two_equal_leaves() {
        // n=600, t=400: 2n+t = 1600 = 2·(2t) → tie, rounds up → k = 2.
        // Leaves [1, 300], [301, 600].
        assert_eq!(leaf_count(600, 400), 2);
        let tree = build_tree(600, 400);
        let [l, r] = children(&tree);
        assert_eq!(l.interval(), [1, 300]);
        assert_eq!(r.interval(), [301, 600]);
        // Line 300 → leaf 0, line 301 → leaf 1.
        assert_eq!(find_leaf(&tree, 300).start, 1);
        assert_eq!(find_leaf(&tree, 301).start, 301);
    }

    #[test]
    fn canonical_n800_t400_two_equal_leaves() {
        // n=800, t=400: k=2 → leaves [1, 400], [401, 800].
        assert_eq!(leaf_count(800, 400), 2);
        let tree = build_tree(800, 400);
        let [l, r] = children(&tree);
        assert_eq!(l.interval(), [1, 400]);
        assert_eq!(r.interval(), [401, 800]);
    }

    #[test]
    fn canonical_n1000_t400_three_leaves() {
        // n=1000, t=400: k=3 → leaves [1, 333], [334, 666], [667, 1000].
        assert_eq!(leaf_count(1000, 400), 3);
        let tree = build_tree(1000, 400);
        for line in 1..=1000 {
            let leaf = find_leaf(&tree, line);
            let j = leaf_index(1000, 3, line);
            assert_eq!(leaf.interval(), leaf_interval(1000, 3, j), "line {line}");
        }
        assert_eq!(find_leaf(&tree, 1).interval(), [1, 333]);
        assert_eq!(find_leaf(&tree, 333).interval(), [1, 333]);
        assert_eq!(find_leaf(&tree, 334).interval(), [334, 666]);
        assert_eq!(find_leaf(&tree, 667).interval(), [667, 1000]);
        assert_eq!(find_leaf(&tree, 1000).interval(), [667, 1000]);
    }

    #[test]
    fn canonical_n1200_t400_three_equal_leaves() {
        // n=1200, t=400: k=3 → leaves [1, 400], [401, 800], [801, 1200].
        assert_eq!(leaf_count(1200, 400), 3);
        let tree = build_tree(1200, 400);
        assert_eq!(find_leaf(&tree, 400).interval(), [1, 400]);
        assert_eq!(find_leaf(&tree, 401).interval(), [401, 800]);
        assert_eq!(find_leaf(&tree, 800).interval(), [401, 800]);
        assert_eq!(find_leaf(&tree, 801).interval(), [801, 1200]);
    }

    // ─── 200,000-case invariant grid (I1–I7, pure integer arithmetic) ──

    /// Target sizes spanning the valid range, including tie points.
    const T_GRID: [usize; 41] = [
        1, 2, 3, 4, 5, 7, 10, 11, 15, 16, 20, 22, 25, 28, 30, 35, 40, 50, 55, 60, 70, 75,
        80, 90, 100, 110, 120, 140, 150, 160, 180, 200, 220, 250, 280, 300, 320, 350, 400,
        500, 1000,
    ];

    /// I1–I7 over the deterministic grid `n ∈ 1..=5000 × T_GRID`
    /// (5000 × 41 = 205,000 cases, satisfying the ≥200k requirement),
    /// no I/O.
    #[test]
    fn invariant_grid_200k() {
        let mut cases = 0usize;
        for &t in &T_GRID {
            // I7: k is non-decreasing in n (for fixed t).
            let mut prev_k = 1usize;
            for n in 1..=5000usize {
                cases += 1;
                let k = leaf_count(n, t);
                let bounds: Vec<usize> = boundaries(n, k);

                // I3: k = 1 ⟺ 2n < 3t (single-leaf rule, both ways).
                if k == 1 {
                    assert!(2 * n < 3 * t, "n={n}, t={t}");
                } else {
                    assert!(2 * n >= 3 * t, "n={n}, t={t}");
                }

                // I5: k ≡ max(1, round_half_up(n/t)) = max(1, floor((2n+t)/(2t))).
                assert_eq!(k, ((2 * n + t) / (2 * t)).max(1), "n={n}, t={t}");

                // I7: monotone in n.
                assert!(k >= prev_k, "k decreased: n={n}, t={t}");
                prev_k = k;

                // I2: rational band on n/k (exact integer comparison;
                // the literal integer size band does not hold under
                // integer division). For k >= 2, k = round(n/t) implies
                // n/k ∈ [3t/4, 3t/2), i.e. 4n >= 3tk and 2n < 3tk.
                if k >= 2 {
                    let (n128, t128, k128) = (n as u128, t as u128, k as u128);
                    assert!(4 * n128 >= 3 * t128 * k128, "n={n}, t={t}, k={k}");
                    assert!(2 * n128 < 3 * t128 * k128, "n={n}, t={t}, k={k}");
                }

                // I1: complete partition — b(0)=0, b(k)=n, strictly
                // increasing; each leaf j = [b(j)+1, b(j+1)] is
                // non-empty and tiles [1, n] in order, and each size is
                // floor(n/k) or ceil(n/k) (I4).
                assert_eq!(bounds[0], 0);
                assert_eq!(bounds[k], n);
                for j in 0..k {
                    assert!(bounds[j] < bounds[j + 1], "n={n}, t={t}, j={j}");
                    let size = bounds[j + 1] - bounds[j];
                    if k == 1 {
                        assert_eq!(size, n);
                    } else {
                        assert!(
                            size == n / k || size == (n + k - 1) / k,
                            "n={n}, t={t}, j={j}, size={size}"
                        );
                    }
                }

                // I6: the closed-form leaf-index oracle `ceil(line·k/n)−1`
                // agrees with the canonical leaf partition at both
                // endpoints of every leaf (the full per-line sweep is
                // covered by `tree_lookup_equals_arithmetic_every_line`).
                for j in 0..k {
                    for line in [bounds[j] + 1, bounds[j + 1]] {
                        assert_eq!(leaf_index(n, k, line), j, "n={n}, t={t}, line={line}");
                    }
                }
            }
        }
        // Deterministic count: 5000 × 41 = 205,000 (n, t) combinations
        // (>= the 200k requirement).
        assert_eq!(cases, 5000 * T_GRID.len());
    }

    // ─── Tree ≡ arithmetic property ─────────────────────────────

    /// For many (n, t) pairs and every line 1..=n, the canonical tree
    /// lookup must return exactly `[b(j)+1, b(j+1)]` where
    /// `j = ceil(line·k/n) − 1`.
    #[test]
    fn tree_lookup_equals_arithmetic_every_line() {
        for n in (1usize..=2000).step_by(7) {
            for t in 1..=60usize {
                let k = leaf_count(n, t);
                let tree = build_tree(n, t);
                for line in 1..=n {
                    let leaf = find_leaf(&tree, line);
                    let j = leaf_index(n, k, line);
                    assert_eq!(
                        leaf.interval(),
                        leaf_interval(n, k, j),
                        "n={n}, t={t}, line={line}"
                    );
                }
            }
        }
    }

    /// SPEC_v0.2.0 §20: the exhaustive tree ≡ arithmetic sweep MUST
    /// include a 10,000-line file.
    #[test]
    fn tree_property_10k_line_file() {
        let n = 10_000usize;
        let t = 400usize;
        let k = leaf_count(n, t);
        assert_eq!(k, 25); // 10000 / 400 = 25 exactly
        let tree = build_tree(n, t);
        for line in 1..=n {
            let j = leaf_index(n, k, line);
            assert_eq!(
                find_leaf(&tree, line).interval(),
                leaf_interval(n, k, j),
                "line {line}"
            );
        }
    }

    #[test]
    fn tree_is_deterministic() {
        for &(n, t) in &[(1usize, 400), (453, 400), (453, 200), (600, 400), (800, 400),
                         (1000, 400), (1200, 400), (5000, 400)] {
            let a = build_tree(n, t);
            let b2 = build_tree(n, t);
            assert_eq!(a, b2, "n={n}, t={t}");
        }
    }

    /// Root always covers `[1, n]`.
    #[test]
    fn root_covers_whole_file() {
        for n in [1usize, 2, 3, 453, 800, 1_000, 5_000] {
            let tree = build_tree(n, 400);
            assert_eq!(tree.interval(), [1, n]);
        }
    }
}
