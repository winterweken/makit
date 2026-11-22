//! Application state — the "Model" of the Elm architecture.
//!
//! `AppState` is plain data: no terminal handles, no channels, no global
//! registry access. Everything that touches the outside world lives in
//! `runtime.rs`; loaders here only run once at startup.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use makit_tools::building_model::WwrAnalysis;
use makit_tools::murb::MurbResults;
use ratatui::layout::Rect;

use crate::node::{NodeId, PreviewKind};
use crate::tree::{Tree, TreeNode};

/// Minimum terminal width for the side-by-side pane layout.
pub const WIDE_BREAKPOINT: u16 = 80;

/// Maximum number of lines kept in the Output pane.
pub const LOG_CAPACITY: usize = 1000;

/// Which pane receives navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    IfcPicker,
    Output,
}

/// A task currently running on a worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningTask {
    pub name: String,
}

/// IFC file picker state.
#[derive(Debug, Clone, Default)]
pub struct IfcState {
    /// Discovered `.ifc` paths.
    pub files: Vec<String>,
    /// Highlighted row in the picker.
    pub selected: usize,
    /// First visible row in the picker.
    pub offset: usize,
    /// Path currently being loaded on a worker thread.
    pub loading: Option<String>,
    /// Last file whose load finished.
    pub loaded: Option<String>,
    /// WWR analysis for `loaded`, when its JSON extraction exists.
    pub analysis: Option<WwrAnalysis>,
}

/// Data gathered once at startup (kept separate so tests can inject it).
#[derive(Debug, Clone, Default)]
pub struct InitialData {
    pub tree: Vec<TreeNode>,
    pub ifc_files: Vec<String>,
    pub murb_results: Option<MurbResults>,
    pub viewport: Rect,
}

/// The whole TUI state.
#[derive(Debug)]
pub struct AppState {
    pub tree: Tree,
    pub focus: Focus,
    pub show_help: bool,
    pub show_output: bool,
    /// Set once the Output pane has auto-opened, so hiding it sticks.
    pub output_auto_opened: bool,
    /// Last node opened with Enter.
    pub opened: Option<NodeId>,
    /// Logo rotation angle (radians).
    pub logo_angle: f64,
    /// Frame counter for spinners.
    pub frame: u64,
    pub status: String,
    /// Terminal size, used for layout-dependent updates and mouse hit-tests.
    pub viewport: Rect,
    pub tasks: BTreeMap<u64, RunningTask>,
    pub next_task_id: u64,
    pub log: VecDeque<String>,
    /// Lines scrolled back from the bottom of the log (0 = follow output).
    pub log_scroll: usize,
    pub ifc: IfcState,
    pub murb_results: Option<MurbResults>,
    pub should_quit: bool,
}

impl AppState {
    /// Build the initial state from startup data.
    pub fn new(data: InitialData) -> Self {
        let status = if data.murb_results.is_some() {
            "Ready — MURB results loaded · ↑↓ navigate · e execute · ? help".to_string()
        } else {
            "Ready — ↑↓ navigate · → expand · e execute · ? help".to_string()
        };
        Self {
            tree: Tree::new(data.tree),
            focus: Focus::Tree,
            show_help: false,
            show_output: false,
            output_auto_opened: false,
            opened: None,
            logo_angle: 0.0,
            frame: 0,
            status,
            viewport: data.viewport,
            tasks: BTreeMap::new(),
            next_task_id: 1,
            log: VecDeque::new(),
            log_scroll: 0,
            ifc: IfcState {
                files: data.ifc_files,
                ..IfcState::default()
            },
            murb_results: data.murb_results,
            should_quit: false,
        }
    }

    /// The node under the explorer cursor.
    pub fn active_node(&self) -> Option<NodeId> {
        self.tree.selected_id().and_then(NodeId::parse)
    }

    /// Description of the node under the explorer cursor.
    pub fn active_description(&self) -> &str {
        self.tree
            .selected_row()
            .map(|r| r.description.as_str())
            .unwrap_or("")
    }

    /// Which preview the detail pane shows.
    pub fn preview_kind(&self) -> PreviewKind {
        PreviewKind::for_node(self.active_node().as_ref())
    }

    /// Whether the IFC picker section is shown in the detail pane.
    pub fn ifc_section_visible(&self) -> bool {
        self.active_node().is_some_and(|n| n.is_ifc_context())
    }

    /// Whether the picker list itself can take focus.
    pub fn ifc_picker_focusable(&self) -> bool {
        self.ifc_section_visible() && !self.ifc.files.is_empty()
    }

    /// Panes that can currently take focus, in Tab order.
    pub fn focus_order(&self) -> Vec<Focus> {
        let mut order = vec![Focus::Tree];
        if self.ifc_picker_focusable() {
            order.push(Focus::IfcPicker);
        }
        if self.show_output {
            order.push(Focus::Output);
        }
        order
    }

    /// Whether the runtime should deliver animation ticks.
    pub fn needs_tick(&self) -> bool {
        self.preview_kind().is_animated() || !self.tasks.is_empty() || self.ifc.loading.is_some()
    }

    /// Append a line to the Output log, dropping the oldest past capacity.
    pub fn push_log(&mut self, line: String) {
        if self.log.len() == LOG_CAPACITY {
            self.log.pop_front();
        }
        if self.log_scroll > 0 {
            // Keep the scrolled-back view anchored while new lines arrive.
            self.log_scroll = (self.log_scroll + 1).min(LOG_CAPACITY);
        }
        self.log.push_back(line);
    }
}

/// Gather startup data: registry tree, IFC files, cached MURB results.
pub fn load_initial_data(viewport: Rect) -> InitialData {
    InitialData {
        tree: crate::tree_data::build_tree_from_global(),
        ifc_files: discover_ifc_files(&[".", "examples/IFC", "examples"]),
        murb_results: load_murb_results(Path::new("murb_results.json")),
        viewport,
    }
}

/// Scan `dirs` (non-recursively) for `.ifc` files, sorted and deduplicated.
pub fn discover_ifc_files(dirs: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ifc"))
            {
                if let Some(s) = path.to_str() {
                    found.push(s.to_string());
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Load cached MURB simulation results, if present and valid.
pub fn load_murb_results(path: &Path) -> Option<MurbResults> {
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        AppState::new(InitialData {
            tree: vec![TreeNode::new("sources", "Sources")
                .child(TreeNode::new("source:ifc", "◆ ifc").description("IFC file loader"))
                .child(TreeNode::new("source:revit", "◆ revit"))],
            ifc_files: vec!["examples/IFC/a.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, 120, 40),
        })
    }

    #[test]
    fn focus_order_tracks_context() {
        let mut s = state();
        assert_eq!(s.focus_order(), vec![Focus::Tree]);
        s.tree.select(1); // source:ifc
        assert!(s.ifc_section_visible());
        assert_eq!(s.focus_order(), vec![Focus::Tree, Focus::IfcPicker]);
        s.show_output = true;
        assert_eq!(
            s.focus_order(),
            vec![Focus::Tree, Focus::IfcPicker, Focus::Output]
        );
        s.ifc.files.clear();
        assert_eq!(s.focus_order(), vec![Focus::Tree, Focus::Output]);
    }

    #[test]
    fn active_node_and_description() {
        let mut s = state();
        s.tree.select(1);
        assert_eq!(s.active_node(), NodeId::parse("source:ifc"));
        assert_eq!(s.active_description(), "IFC file loader");
    }

    #[test]
    fn ticks_only_when_something_moves() {
        let mut s = state();
        s.tree.select(0); // logo preview animates
        assert!(s.needs_tick());
        s.tree.select(2); // revit → static building preview
        assert!(!s.needs_tick());
        s.tasks.insert(1, RunningTask { name: "x".into() });
        assert!(s.needs_tick());
    }

    #[test]
    fn log_is_capped_and_scroll_anchor_moves() {
        let mut s = state();
        for i in 0..LOG_CAPACITY + 5 {
            s.push_log(format!("line {i}"));
        }
        assert_eq!(s.log.len(), LOG_CAPACITY);
        assert_eq!(s.log.front().unwrap(), "line 5");

        let mut s = state();
        s.push_log("a".into());
        s.log_scroll = 1;
        s.push_log("b".into());
        assert_eq!(s.log_scroll, 2);

        // Also anchored once the log is full.
        let mut s = state();
        for i in 0..LOG_CAPACITY {
            s.push_log(format!("{i}"));
        }
        s.log_scroll = 5;
        s.push_log("new".into());
        assert_eq!(s.log_scroll, 6);
    }

    #[test]
    fn discovers_example_ifc_files() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/IFC");
        let files = discover_ifc_files(&[root, root]);
        assert!(!files.is_empty(), "examples/IFC should contain .ifc files");
        assert!(files.iter().all(|f| f.to_lowercase().ends_with(".ifc")));
        let mut sorted = files.clone();
        sorted.dedup();
        assert_eq!(files, sorted);
    }

    #[test]
    fn missing_murb_results_is_none() {
        assert!(load_murb_results(Path::new("/definitely/not/here.json")).is_none());
    }
}
