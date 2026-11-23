//! Messages (inputs to `update`) and commands (side effects requested by it).

use std::collections::BTreeMap;

use makit_tools::building_model::{Envelope, WwrAnalysis};
use makit_tools::murb::MurbResults;

use crate::node::NodeId;

/// A pane, used for mouse targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Tree,
    IfcPicker,
    Output,
    /// Anywhere else in the detail pane (not focusable).
    Detail,
}

/// Result of loading an IFC selection on a worker thread.
// Built once per load; boxing the analysis would only move the allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum IfcOutcome {
    /// The JSON extraction was found (or made) and analysed.
    Analysis {
        analysis: WwrAnalysis,
        /// The MURB envelope at grade 0.
        envelope: Envelope,
        /// Path of the JSON extraction the analysis came from.
        json: String,
        /// The extractor ran for this load.
        extracted: bool,
        /// Why a stale extraction had to do, when it did.
        note: Option<String>,
    },
    /// No JSON extraction exists next to the IFC file and none could be
    /// made; `reason` says why.
    NeedsExtraction { reason: String },
}

/// A key press while the option editor is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    /// Empty the field.
    Clear,
    /// Accept the value.
    Submit,
    /// Close the editor, keeping the old value.
    Cancel,
}

/// Everything that can happen to the app.
// The two worker results (an IFC analysis with its envelope, a boxed MURB
// result) dwarf the key and tick messages; they arrive a handful of times per
// session, so boxing them too would only move the allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Msg {
    // Navigation within the focused pane.
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Expand,
    Collapse,
    /// Enter: open/toggle a tree node, edit an option, or load the
    /// highlighted IFC file.
    Activate,
    /// Run the task for the node under the explorer cursor, asking for any
    /// required option that has no value yet.
    Execute,
    /// Ask the focused (or only) running task to stop.
    StopTask,
    /// A key press in the option editor.
    Edit(EditKey),
    FocusNext,
    FocusPrev,
    ToggleOutput,
    ClearOutput,
    ToggleHelp,
    /// Esc: close the help overlay, otherwise quit.
    Escape,
    Quit,
    // Mouse.
    Scroll {
        pane: Pane,
        delta: isize,
    },
    Click {
        pane: Pane,
        row: Option<usize>,
    },
    // Terminal and timers.
    Resize {
        width: u16,
        height: u16,
    },
    Tick,
    // Worker results.
    Output(String),
    TaskFinished {
        id: u64,
        result: Result<String, String>,
    },
    /// The IFC worker is about to run the extractor on `path`.
    IfcExtracting {
        path: String,
    },
    IfcLoaded {
        path: String,
        result: Result<IfcOutcome, String>,
    },
    /// Simulation results re-read from disk (`None` if unreadable), boxed:
    /// they carry the run's whole context and would dwarf every other message.
    MurbLoaded(Option<Box<MurbResults>>),
}

/// Side effects `update` asks the runtime to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// Resolve the task `node` against the registry and run it on a worker
    /// thread with `options` laid over the registered defaults.
    RunTask {
        id: u64,
        node: NodeId,
        options: BTreeMap<String, String>,
    },
    /// Ask a running task to stop (only tasks that watch for it will).
    StopTask { id: u64 },
    /// Load and analyse an IFC selection on a worker thread.
    LoadIfc { path: String },
    /// Re-read simulation results from `path` on a worker thread.
    LoadMurb { path: String },
    /// Exit the event loop.
    Quit,
}
