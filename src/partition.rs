//! Deterministic binary partition tree (SPEC §6).
//!
//! The tree is an internal implementation detail and MUST NOT be exposed
//! as part of the public interface (SPEC §5). It is reconstructed on every
//! invocation from the current file (SPEC §11: computed view, not an index).

use std::rc::Rc;

/// A node in the partition tree: a contiguous 1-based inclusive line interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionNode {
    /// First line of this partition (1-based, inclusive).
    pub start: usize,
    /// Last line of this partition (1-based, inclusive).
    pub end: usize,
    /// Children, if this node was split.
    pub children: Option<(Rc<PartitionNode>, Rc<PartitionNode>)>,
}

#[allow(dead_code)] // used by tests and future scope-resolution strategies
impl PartitionNode {
    /// Number of lines in this interval.
    pub fn len(&self) -> usize {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The root of the partition tree for a file.
pub type PartitionTree = Rc<PartitionNode>;

/// Recursively binary-partition `[start, end]` (1-based, inclusive) until
/// every leaf holds at most `target` lines.
///
/// Deterministic balancing rule (SPEC §6.1): each split divides the interval
/// into a left child of `len / 2` lines and a right child of `len - len / 2`
/// lines. The resulting tree is a pure function of the file's line count and
/// the target width, so it is deterministic for the same file contents and
/// configuration (SPEC §18).
fn build(start: usize, end: usize, target: usize) -> Rc<PartitionNode> {
    let len = end - start + 1;
    if len <= target {
        return Rc::new(PartitionNode {
            start,
            end,
            children: None,
        });
    }
    let left_len = len / 2;
    let left = build(start, start + left_len - 1, target);
    let right = build(start + left_len, end, target);
    Rc::new(PartitionNode {
        start,
        end,
        children: Some((left, right)),
    })
}

/// Build the partition tree for a file with `line_count` lines.
///
/// `line_count` MUST be > 0.
pub fn build_tree(line_count: usize, target: usize) -> PartitionTree {
    assert!(line_count > 0, "line_count must be positive");
    assert!(target > 0, "target must be positive");
    build(1, line_count, target)
}

/// Locate the leaf partition containing `line` (1-based).
pub fn find_leaf(root: &PartitionNode, line: usize) -> &PartitionNode {
    let mut node = root;
    loop {
        match &node.children {
            None => return node,
            Some((left, right)) => {
                node = if line <= left.end { left } else { right };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_file_is_a_leaf() {
        let tree = build_tree(1, 50);
        assert_eq!(tree.start, 1);
        assert_eq!(tree.end, 1);
        assert!(tree.children.is_none());
    }

    #[test]
    fn small_file_is_single_leaf() {
        let tree = build_tree(30, 50);
        assert!(tree.children.is_none());
        assert_eq!(tree.len(), 30);
    }

    #[test]
    fn leaves_cover_file_exactly() {
        let tree = build_tree(1193, 50);
        let mut leaves: Vec<&PartitionNode> = Vec::new();
        collect_leaves(&tree, &mut leaves);
        assert!(!leaves.is_empty());
        let mut expected_start = 1;
        for leaf in &leaves {
            assert_eq!(leaf.start, expected_start);
            assert!(leaf.len() <= 50, "leaf too wide: {:?}", leaf);
            assert!(leaf.children.is_none());
            expected_start = leaf.end + 1;
        }
        assert_eq!(expected_start - 1, 1193);
    }

    fn collect_leaves<'a>(node: &'a PartitionNode, out: &mut Vec<&'a PartitionNode>) {
        match &node.children {
            None => out.push(node),
            Some((l, r)) => {
                collect_leaves(l, out);
                collect_leaves(r, out);
            }
        }
    }

    #[test]
    fn find_leaf_locates_line() {
        let tree = build_tree(1193, 50);
        for line in 1..=1193 {
            let leaf = find_leaf(&tree, line);
            assert!(leaf.start <= line && line <= leaf.end, "line {line}");
        }
    }

    #[test]
    fn boundary_lines() {
        let tree = build_tree(100, 50);
        // Split: [1,50] / [51,100].
        let left = find_leaf(&tree, 50);
        assert_eq!((left.start, left.end), (1, 50));
        let right = find_leaf(&tree, 51);
        assert_eq!((right.start, right.end), (51, 100));
    }

    #[test]
    fn odd_line_counts() {
        let tree = build_tree(101, 50);
        // First split: [1,50] / [51,101]; [51,101] has 51 lines > 50, so it
        // splits again: [51,75] / [76,101].
        let r_left = find_leaf(&tree, 51);
        assert_eq!((r_left.start, r_left.end), (51, 75));
        let r_right = find_leaf(&tree, 101);
        assert_eq!((r_right.start, r_right.end), (76, 101));
    }

    #[test]
    fn determinism_same_inputs_same_tree() {
        let a = build_tree(1000, 50);
        let b = build_tree(1000, 50);
        assert_eq!(dump(&a), dump(&b));
    }

    fn dump(node: &PartitionNode) -> String {
        let mut s = format!("{}-{}", node.start, node.end);
        if let Some((l, r)) = &node.children {
            s.push('(');
            s.push_str(&dump(l));
            s.push(' ');
            s.push_str(&dump(r));
            s.push(')');
        }
        s
    }
}
