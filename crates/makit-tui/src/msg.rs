//! Messages (inputs to `update`) and commands (side effects requested by it).

use makit_tools::building_model::WwrAnalysis;

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
#[derive(Debug, Clone)]
pub enum IfcOutcome {
    /// The JSON extraction was found and analysed.
    Analysis(WwrAnalysis),
    /// No JSON extraction exists next to the IFC file yet.
    NeedsExtraction,
}

/// Everything that can happen to the app.
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
    /// Enter: open/toggle a tree node, or load the highlighted IFC file.
    Activate,
    /// Run the task for the node under the explorer cursor.
    Execute,
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
    IfcLoaded {
        path: String,
        result: Result<IfcOutcome, String>,
    },
}

/// Side effects `update` asks the runtime to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// Resolve `node` against the registry and run it on a worker thread.
    RunTask { id: u64, node: NodeId },
    /// Load and analyse an IFC selection on a worker thread.
    LoadIfc { path: String },
    /// Exit the event loop.
    Quit,
}
