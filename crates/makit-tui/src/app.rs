//! Application state — the "Model" of the Elm architecture.
//!
//! `AppState` is plain data: no terminal handles, no channels, no global
//! registry access. Everything that touches the outside world lives in
//! `runtime.rs`; loaders here only run once at startup.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;

use makit_tools::building_model::{Envelope, WwrAnalysis};
use makit_tools::murb::MurbResults;
use ratatui::layout::Rect;

use crate::node::{NodeId, PickedFile, PreviewKind};
use crate::options::Catalog;
use crate::preview::PreviewData;
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
    /// The source or action node that was run.
    pub node: NodeId,
    /// The user asked for it to stop.
    pub stopping: bool,
    /// Results file to re-read when the task succeeds.
    pub results: Option<String>,
    /// IFC file to reload in the picker when the task succeeds (the `ifc`
    /// source's `--file`).
    pub reload_ifc: Option<String>,
}

impl RunningTask {
    /// A freshly started task for `node`.
    pub fn new(node: NodeId) -> Self {
        Self {
            name: node.display_name().to_string(),
            node,
            stopping: false,
            results: None,
            reload_ifc: None,
        }
    }
}

/// Where an option's current value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueSource {
    /// Typed by the user.
    User,
    /// Taken from the IFC picker's loaded file.
    Picker,
    /// The registered default.
    Default,
}

/// The single-line form for typing an option value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// The source or action the option belongs to.
    pub task: NodeId,
    pub opt: String,
    pub buffer: String,
    /// Cursor position, in characters from the start of `buffer`.
    pub cursor: usize,
    /// Run the task once the value is accepted (the editor was opened
    /// because the task could not run without it).
    pub run_after: bool,
}

impl Editor {
    /// Open the form on `task --opt`, with `initial` ready to edit.
    pub fn new(task: NodeId, opt: impl Into<String>, initial: &str, run_after: bool) -> Self {
        Self {
            task,
            opt: opt.into(),
            buffer: initial.to_string(),
            cursor: initial.chars().count(),
            run_after,
        }
    }

    fn len(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte offset of character `index` in the buffer.
    fn byte_at(&self, index: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(index)
            .map_or(self.buffer.len(), |(byte, _)| byte)
    }

    /// Insert `c` at the cursor.
    pub fn insert(&mut self, c: char) {
        let at = self.byte_at(self.cursor);
        self.buffer.insert(at, c);
        self.cursor += 1;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.delete();
        }
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.len() {
            let at = self.byte_at(self.cursor);
            self.buffer.remove(at);
        }
    }

    /// Move the cursor by `delta` characters, staying inside the text.
    pub fn move_by(&mut self, delta: isize) {
        self.cursor = self.cursor.saturating_add_signed(delta).min(self.len());
    }

    /// Move the cursor to the start.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end.
    pub fn end(&mut self) {
        self.cursor = self.len();
    }

    /// Empty the field.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }
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
    /// The MURB envelope of `loaded` at grade 0, with `analysis`.
    pub envelope: Option<Envelope>,
    /// Path of the JSON extraction `analysis` was read from.
    pub json: Option<String>,
    /// Why loading `loaded` failed, if it did.
    pub error: Option<String>,
    /// Why `loaded` has no extraction, or why a stale one is in use.
    pub note: Option<String>,
}

/// Data gathered once at startup (kept separate so tests can inject it).
#[derive(Debug, Clone, Default)]
pub struct InitialData {
    pub tree: Vec<TreeNode>,
    pub ifc_files: Vec<String>,
    pub murb_results: Option<MurbResults>,
    /// Options of every source and action.
    pub catalog: Catalog,
    pub viewport: Rect,
}

/// The whole TUI state.
#[derive(Debug)]
pub struct AppState {
    pub tree: Tree,
    pub focus: Focus,
    pub show_help: bool,
    /// Lines the help overlay is scrolled down by.
    pub help_scroll: u16,
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
    /// Options of every source and action.
    pub catalog: Catalog,
    /// Option values the user entered, by task.
    pub values: HashMap<NodeId, BTreeMap<String, String>>,
    /// The open option form, if any (modal).
    pub editor: Option<Editor>,
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
            help_scroll: 0,
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
            catalog: data.catalog,
            values: HashMap::new(),
            editor: None,
            should_quit: false,
        }
    }

    /// The current value of `task --opt` and where it comes from: what the
    /// user typed, else the IFC picker's file (for the option bound to it),
    /// else the registered default. Empty values count as unset.
    pub fn option_value(&self, task: &NodeId, opt: &str) -> Option<(String, ValueSource)> {
        let typed = self.values.get(task).and_then(|v| v.get(opt));
        if let Some(value) = typed.filter(|v| !v.is_empty()) {
            return Some((value.clone(), ValueSource::User));
        }
        if let Some(value) = self.picked_value(task, opt) {
            return Some((value, ValueSource::Picker));
        }
        self.catalog
            .spec(task, opt)
            .and_then(|spec| spec.default.clone())
            .filter(|v| !v.is_empty())
            .map(|v| (v, ValueSource::Default))
    }

    /// The IFC picker's contribution to `task --opt`, if it is bound to it.
    fn picked_value(&self, task: &NodeId, opt: &str) -> Option<String> {
        let (bound, which) = task.picker_option()?;
        if bound != opt {
            return None;
        }
        match which {
            PickedFile::Ifc => self.ifc.loaded.clone(),
            PickedFile::Json => self.ifc.json.clone(),
        }
    }

    /// Every option of `task` that has a value, ready to run with.
    pub fn resolved_options(&self, task: &NodeId) -> BTreeMap<String, String> {
        self.catalog
            .options(task)
            .iter()
            .filter_map(|spec| {
                self.option_value(task, &spec.name)
                    .map(|(value, _)| (spec.name.clone(), value))
            })
            .collect()
    }

    /// Required options of `task` that still have no value, in
    /// registration order.
    pub fn missing_required(&self, task: &NodeId) -> Vec<String> {
        self.catalog
            .options(task)
            .iter()
            .filter(|spec| spec.required && self.option_value(task, &spec.name).is_none())
            .map(|spec| spec.name.clone())
            .collect()
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
        let kind = PreviewKind::for_node(self.active_node().as_ref());
        // The IFC source has no drawing of its own: once a file has been
        // analysed, its walls are worth more than the logo.
        if kind == PreviewKind::Logo && self.preview_data().wwr.is_some() {
            PreviewKind::Walls
        } else {
            kind
        }
    }

    /// The real data the preview can draw for the focused node.
    pub fn preview_data(&self) -> PreviewData<'_> {
        PreviewData {
            murb: self.murb_results.as_ref(),
            // The analysis describes the picked file, so it only belongs
            // to nodes that work on that file.
            wwr: self
                .ifc
                .analysis
                .as_ref()
                .filter(|_| self.ifc_section_visible()),
        }
    }

    /// Whether the IFC picker section is shown in the detail pane.
    pub fn ifc_section_visible(&self) -> bool {
        self.active_node().is_some_and(|n| n.is_ifc_context())
    }

    /// Whether the picker list itself can take focus: it has files to
    /// offer and the screen has room to draw it.
    pub fn ifc_picker_focusable(&self) -> bool {
        self.ifc_section_visible()
            && !self.ifc.files.is_empty()
            && crate::layout::compute(self.viewport, self)
                .ifc
                .is_some_and(|ifc| ifc.list.is_some())
    }

    /// Panes that can currently take focus, in Tab order.
    pub fn focus_order(&self) -> Vec<Focus> {
        let mut order = vec![Focus::Tree];
        if self.ifc_picker_focusable() {
            order.push(Focus::IfcPicker);
        }
        if self.output_visible() {
            order.push(Focus::Output);
        }
        order
    }

    /// Whether the Output pane is on screen: switched on, and the terminal
    /// is tall enough to fit it.
    pub fn output_visible(&self) -> bool {
        self.show_output && crate::layout::output_height(self.viewport.height) > 0
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
        catalog: Catalog::from_global(),
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
    use crate::options::OptionSpec;

    fn state() -> AppState {
        AppState::new(InitialData {
            tree: vec![TreeNode::new("sources", "Sources")
                .child(TreeNode::new("source:ifc", "◆ ifc").description("IFC file loader"))
                .child(TreeNode::new("source:revit", "◆ revit"))],
            ifc_files: vec!["examples/IFC/a.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, 120, 40),
            ..InitialData::default()
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
        s.tasks
            .insert(1, RunningTask::new(NodeId::Source("revit".into())));
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

    // ─── Option values ──────────────────────────────────────────────────

    fn ifc() -> NodeId {
        NodeId::Source("ifc".into())
    }

    fn wwr() -> NodeId {
        NodeId::Action("wall-orientation-wwr".into())
    }

    fn with_catalog() -> AppState {
        let spec = |name: &str, required: bool, default: Option<&str>| OptionSpec {
            name: name.into(),
            description: String::new(),
            kind: "string".into(),
            required,
            default: default.map(str::to_string),
        };
        let mut s = state();
        s.catalog.insert(ifc(), vec![spec("file", true, None)]);
        s.catalog.insert(
            wwr(),
            vec![
                spec("input", true, None),
                spec("unit", false, Some("sqm")),
                spec("output", false, Some("")),
            ],
        );
        s
    }

    #[test]
    fn option_values_come_from_the_user_then_the_picker_then_defaults() {
        let mut s = with_catalog();
        assert_eq!(s.option_value(&wwr(), "input"), None);
        assert_eq!(
            s.option_value(&wwr(), "unit"),
            Some(("sqm".to_string(), ValueSource::Default))
        );
        // An empty default is no value at all.
        assert_eq!(s.option_value(&wwr(), "output"), None);

        s.ifc.loaded = Some("m/a.ifc".into());
        s.ifc.json = Some("m/building-model.json".into());
        assert_eq!(
            s.option_value(&ifc(), "file"),
            Some(("m/a.ifc".to_string(), ValueSource::Picker))
        );
        assert_eq!(
            s.option_value(&wwr(), "input"),
            Some(("m/building-model.json".to_string(), ValueSource::Picker))
        );
        // The picker only fills the option it is bound to.
        assert_eq!(
            s.option_value(&wwr(), "unit"),
            Some(("sqm".to_string(), ValueSource::Default))
        );

        s.values
            .entry(wwr())
            .or_default()
            .insert("unit".into(), "sqf".into());
        assert_eq!(
            s.option_value(&wwr(), "unit"),
            Some(("sqf".to_string(), ValueSource::User))
        );
    }

    #[test]
    fn missing_required_lists_only_unset_required_options() {
        let mut s = with_catalog();
        assert_eq!(s.missing_required(&wwr()), vec!["input".to_string()]);
        // An IFC without a JSON extraction does not satisfy `--input`.
        s.ifc.loaded = Some("m/a.ifc".into());
        assert_eq!(s.missing_required(&wwr()), vec!["input".to_string()]);
        assert!(s.missing_required(&ifc()).is_empty());
        s.ifc.json = Some("m/building-model.json".into());
        assert!(s.missing_required(&wwr()).is_empty());
        // Unknown tasks have nothing to ask for.
        assert!(s
            .missing_required(&NodeId::Action("nope".into()))
            .is_empty());
    }

    #[test]
    fn resolved_options_hold_every_known_value() {
        let mut s = with_catalog();
        s.ifc.json = Some("m/building-model.json".into());
        s.values
            .entry(wwr())
            .or_default()
            .insert("output".into(), "out.json".into());
        let got = s.resolved_options(&wwr());
        let want: BTreeMap<String, String> = [
            ("input", "m/building-model.json"),
            ("output", "out.json"),
            ("unit", "sqm"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        assert_eq!(got, want);
    }

    // ─── Editor ─────────────────────────────────────────────────────────

    #[test]
    fn editor_starts_with_the_cursor_at_the_end() {
        let e = Editor::new(ifc(), "file", "héllo", false);
        assert_eq!(e.buffer, "héllo");
        assert_eq!(e.cursor, 5);
    }

    #[test]
    fn editor_edits_at_the_cursor_by_character() {
        let mut e = Editor::new(ifc(), "file", "añb", false);
        e.move_by(-1); // between ñ and b
        e.insert('→');
        assert_eq!(e.buffer, "añ→b");
        assert_eq!(e.cursor, 3);
        e.backspace();
        assert_eq!(e.buffer, "añb");
        e.backspace();
        assert_eq!(e.buffer, "ab");
        assert_eq!(e.cursor, 1);
        e.delete();
        assert_eq!(e.buffer, "a");
        // Nothing left to the right, nothing happens.
        e.delete();
        assert_eq!(e.buffer, "a");
        e.home();
        e.backspace();
        assert_eq!((e.buffer.as_str(), e.cursor), ("a", 0));
        e.move_by(-3);
        assert_eq!(e.cursor, 0);
        e.move_by(9);
        assert_eq!(e.cursor, 1);
        e.home();
        e.end();
        assert_eq!(e.cursor, 1);
        e.clear();
        assert_eq!((e.buffer.as_str(), e.cursor), ("", 0));
    }

    // ─── Preview data ───────────────────────────────────────────────────

    fn analysis() -> WwrAnalysis {
        WwrAnalysis {
            orientations: Vec::new(),
            total_wall_area: 10.0,
            total_window_area: 2.0,
            overall_wwr: 20.0,
            wall_count: 1,
            window_count: 1,
            project_name: String::new(),
            interior_walls_left_out: 0,
        }
    }

    #[test]
    fn a_loaded_analysis_replaces_the_logo_on_ifc_nodes() {
        let mut s = state();
        s.tree.select(1); // source:ifc
        assert_eq!(s.preview_kind(), PreviewKind::Logo);
        assert!(s.preview_data().wwr.is_none());

        s.ifc.analysis = Some(analysis());
        assert_eq!(s.preview_kind(), PreviewKind::Walls);
        assert!(s.preview_data().wwr.is_some());
        // A drawing of real data does not need animation ticks.
        assert!(!s.needs_tick());
    }

    #[test]
    fn the_analysis_is_not_shown_for_unrelated_nodes() {
        let mut s = state();
        s.ifc.analysis = Some(analysis());
        s.tree.select(2); // source:revit — the IFC analysis says nothing about it
        assert_eq!(s.preview_kind(), PreviewKind::Building);
        assert!(s.preview_data().wwr.is_none());
        s.tree.select(0); // the Sources group keeps the logo
        assert_eq!(s.preview_kind(), PreviewKind::Logo);
    }
}
