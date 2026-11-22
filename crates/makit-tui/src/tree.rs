//! A small expandable tree model for the explorer pane.
//!
//! The tree is stored as nested [`TreeNode`]s; navigation operates on the
//! flattened list of currently visible rows, which is cached and rebuilt only
//! when nodes are expanded or collapsed.

use std::collections::HashSet;

/// A node in the explorer tree.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub description: String,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Create a leaf node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            children: Vec::new(),
        }
    }

    /// Set the description shown in the detail pane.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Append a child node.
    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }
}

/// A row of the flattened, currently-visible tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    pub id: String,
    pub label: String,
    pub description: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

/// Explorer tree: nodes plus selection, expansion and scroll state.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    nodes: Vec<TreeNode>,
    expanded: HashSet<String>,
    rows: Vec<VisibleRow>,
    selected: usize,
    offset: usize,
}

impl Tree {
    /// Build a tree with every top-level node expanded.
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let expanded = nodes
            .iter()
            .filter(|n| !n.children.is_empty())
            .map(|n| n.id.clone())
            .collect();
        let mut tree = Self {
            nodes,
            expanded,
            ..Self::default()
        };
        tree.rebuild_rows();
        tree
    }

    /// Visible rows, top to bottom.
    pub fn rows(&self) -> &[VisibleRow] {
        &self.rows
    }

    /// Index of the selected row.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// First visible row index (scroll position).
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The selected row, if the tree is non-empty.
    pub fn selected_row(&self) -> Option<&VisibleRow> {
        self.rows.get(self.selected)
    }

    /// Id of the selected row.
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_row().map(|r| r.id.as_str())
    }

    /// Select a row by index (clamped).
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.rows.len().saturating_sub(1));
    }

    /// Move the selection by `delta` rows (clamped at both ends).
    pub fn move_by(&mut self, delta: isize) {
        let target = self.selected.saturating_add_signed(delta);
        self.select(target);
    }

    /// Select the first row.
    pub fn first(&mut self) {
        self.select(0);
    }

    /// Select the last row.
    pub fn last(&mut self) {
        self.select(usize::MAX);
    }

    /// Expand the selected node. Returns `true` if anything changed.
    pub fn expand(&mut self) -> bool {
        match self.selected_row() {
            Some(row) if row.has_children && !row.expanded => {
                let id = row.id.clone();
                self.expanded.insert(id);
                self.rebuild_rows();
                true
            }
            _ => false,
        }
    }

    /// Collapse the selected node, or jump to its parent if it is already
    /// collapsed (or a leaf).
    pub fn collapse_or_parent(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.expanded {
            let id = row.id.clone();
            self.expanded.remove(&id);
            self.rebuild_rows();
        } else if row.depth > 0 {
            let depth = row.depth;
            if let Some(parent) = self.rows[..self.selected]
                .iter()
                .rposition(|r| r.depth < depth)
            {
                self.selected = parent;
            }
        }
    }

    /// Toggle expansion of the selected node. Returns `true` if it has children.
    pub fn toggle(&mut self) -> bool {
        match self.selected_row() {
            Some(row) if row.has_children => {
                let id = row.id.clone();
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
                self.rebuild_rows();
                true
            }
            _ => false,
        }
    }

    /// Adjust the scroll offset so the selection is inside a window of
    /// `height` rows.
    pub fn scroll_to_selected(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
        let max_offset = self.rows.len().saturating_sub(height);
        self.offset = self.offset.min(max_offset);
    }

    /// Row index displayed at `y` rows below the top of the list, if any.
    pub fn row_at(&self, y: usize) -> Option<usize> {
        let index = self.offset + y;
        (index < self.rows.len()).then_some(index)
    }

    fn rebuild_rows(&mut self) {
        let selected_id = self.selected_id().map(str::to_owned);
        let mut rows = Vec::new();
        flatten(&self.nodes, 0, &self.expanded, &mut rows);
        self.rows = rows;
        self.selected = selected_id
            .and_then(|id| self.rows.iter().position(|r| r.id == id))
            .unwrap_or(self.selected)
            .min(self.rows.len().saturating_sub(1));
    }
}

fn flatten(
    nodes: &[TreeNode],
    depth: usize,
    expanded: &HashSet<String>,
    out: &mut Vec<VisibleRow>,
) {
    for node in nodes {
        let is_expanded = !node.children.is_empty() && expanded.contains(&node.id);
        out.push(VisibleRow {
            id: node.id.clone(),
            label: node.label.clone(),
            description: node.description.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded: is_expanded,
        });
        if is_expanded {
            flatten(&node.children, depth + 1, expanded, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// sources ─ ifc ─ --path
    ///         └ revit
    /// actions ─ analysis ─ murb
    fn sample() -> Tree {
        Tree::new(vec![
            TreeNode::new("sources", "Sources")
                .child(
                    TreeNode::new("source:ifc", "◆ ifc")
                        .child(TreeNode::new("source:ifc:opt:path", "--path")),
                )
                .child(TreeNode::new("source:revit", "◆ revit")),
            TreeNode::new("actions", "Actions").child(
                TreeNode::new("cat:analysis", "⊞ analysis")
                    .child(TreeNode::new("action:murb", "▸ murb")),
            ),
        ])
    }

    fn ids(t: &Tree) -> Vec<&str> {
        t.rows().iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn top_level_nodes_start_expanded() {
        let t = sample();
        assert_eq!(
            ids(&t),
            vec![
                "sources",
                "source:ifc",
                "source:revit",
                "actions",
                "cat:analysis"
            ]
        );
        assert_eq!(t.rows()[1].depth, 1);
        assert!(t.rows()[0].expanded);
        assert!(!t.rows()[1].expanded);
        assert!(t.rows()[1].has_children);
        assert!(!t.rows()[2].has_children);
    }

    #[test]
    fn movement_is_clamped() {
        let mut t = sample();
        t.move_by(-5);
        assert_eq!(t.selected(), 0);
        t.move_by(2);
        assert_eq!(t.selected_id(), Some("source:revit"));
        t.move_by(100);
        assert_eq!(t.selected_id(), Some("cat:analysis"));
        t.first();
        assert_eq!(t.selected(), 0);
        t.last();
        assert_eq!(t.selected(), t.rows().len() - 1);
    }

    #[test]
    fn expand_reveals_children_and_keeps_selection() {
        let mut t = sample();
        t.select(1); // source:ifc
        assert!(t.expand());
        assert_eq!(t.rows()[2].id, "source:ifc:opt:path");
        assert_eq!(t.rows()[2].depth, 2);
        assert_eq!(t.selected_id(), Some("source:ifc"));
        // Expanding again or expanding a leaf is a no-op.
        assert!(!t.expand());
        t.select(2);
        assert!(!t.expand());
    }

    #[test]
    fn collapse_then_jump_to_parent() {
        let mut t = sample();
        t.select(1);
        t.expand();
        t.select(2); // --path (leaf)
        t.collapse_or_parent();
        assert_eq!(t.selected_id(), Some("source:ifc"));
        t.collapse_or_parent(); // collapse ifc
        assert!(!t.selected_row().unwrap().expanded);
        assert!(!ids(&t).contains(&"source:ifc:opt:path"));
        t.collapse_or_parent(); // jump to "sources"
        assert_eq!(t.selected_id(), Some("sources"));
        t.collapse_or_parent(); // collapse root ("actions" stays expanded)
        assert_eq!(ids(&t), vec!["sources", "actions", "cat:analysis"]);
        assert_eq!(t.selected_id(), Some("sources"));
    }

    #[test]
    fn collapsing_keeps_selection_on_same_id_when_rows_shift() {
        let mut t = sample();
        t.select(3); // actions
        t.select(0);
        t.toggle(); // collapse sources: actions moves up
        t.select(1);
        assert_eq!(t.selected_id(), Some("actions"));
        t.select(0);
        t.toggle(); // re-expand
        assert_eq!(t.selected_id(), Some("sources"));
    }

    #[test]
    fn toggle_reports_leaves() {
        let mut t = sample();
        t.select(2); // source:revit
        assert!(!t.toggle());
        t.select(4); // cat:analysis
        assert!(t.toggle());
        assert_eq!(t.rows().last().unwrap().id, "action:murb");
    }

    #[test]
    fn scroll_window_follows_selection() {
        let mut t = sample();
        t.select(4);
        t.scroll_to_selected(2);
        assert_eq!(t.offset(), 3);
        assert_eq!(t.row_at(0), Some(3));
        assert_eq!(t.row_at(1), Some(4));
        assert_eq!(t.row_at(2), None);
        t.select(0);
        t.scroll_to_selected(2);
        assert_eq!(t.offset(), 0);
        // A window taller than the tree never scrolls.
        t.select(4);
        t.scroll_to_selected(50);
        assert_eq!(t.offset(), 0);
    }

    #[test]
    fn empty_tree_is_safe() {
        let mut t = Tree::new(Vec::new());
        t.move_by(1);
        t.last();
        assert!(!t.expand());
        assert!(!t.toggle());
        t.collapse_or_parent();
        t.scroll_to_selected(10);
        assert_eq!(t.selected_id(), None);
        assert_eq!(t.row_at(0), None);
    }
}
