//! Typed tree-node identifiers and preview routing.
//!
//! Tree rows carry string ids (`source:ifc`, `action:murb-simulate:opt:climate`,
//! `cat:analysis`, …). [`NodeId`] parses them once so the rest of the TUI can
//! match on structure instead of substrings.

use std::fmt;

/// Top-level groups in the explorer tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Sources,
    Actions,
}

/// Which path of the IFC picker's selection an option wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickedFile {
    /// The `.ifc` file itself.
    Ifc,
    /// Its JSON extraction (`building-model.json`).
    Json,
}

/// A parsed explorer-tree node id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    /// `sources` / `actions`
    Group(Group),
    /// `cat:{name}`
    Category(String),
    /// `source:{name}`
    Source(String),
    /// `source:{source}:opt:{opt}`
    SourceOpt { source: String, opt: String },
    /// `action:{name}`
    Action(String),
    /// `action:{action}:opt:{opt}`
    ActionOpt { action: String, opt: String },
}

impl NodeId {
    /// Parse a tree id string. Returns `None` for unknown formats.
    pub fn parse(id: &str) -> Option<NodeId> {
        match id {
            "sources" => return Some(NodeId::Group(Group::Sources)),
            "actions" => return Some(NodeId::Group(Group::Actions)),
            _ => {}
        }
        if let Some(name) = id.strip_prefix("cat:") {
            return non_empty(name).map(|n| NodeId::Category(n.to_string()));
        }
        if let Some(rest) = id.strip_prefix("source:") {
            return Some(match rest.split_once(":opt:") {
                Some((source, opt)) => NodeId::SourceOpt {
                    source: non_empty(source)?.to_string(),
                    opt: non_empty(opt)?.to_string(),
                },
                None => NodeId::Source(non_empty(rest)?.to_string()),
            });
        }
        if let Some(rest) = id.strip_prefix("action:") {
            return Some(match rest.split_once(":opt:") {
                Some((action, opt)) => NodeId::ActionOpt {
                    action: non_empty(action)?.to_string(),
                    opt: non_empty(opt)?.to_string(),
                },
                None => NodeId::Action(non_empty(rest)?.to_string()),
            });
        }
        None
    }

    /// Short human-facing name: the option name for option nodes, otherwise
    /// the node's own name.
    pub fn display_name(&self) -> &str {
        match self {
            NodeId::Group(Group::Sources) => "sources",
            NodeId::Group(Group::Actions) => "actions",
            NodeId::Category(name) | NodeId::Source(name) | NodeId::Action(name) => name,
            NodeId::SourceOpt { opt, .. } | NodeId::ActionOpt { opt, .. } => opt,
        }
    }

    /// Name for headings: `task --option` for option nodes, otherwise the
    /// node's own name.
    pub fn label(&self) -> String {
        match (self.task_name(), self.opt_name()) {
            (Some(task), Some(opt)) => format!("{task} --{opt}"),
            _ => self.display_name().to_string(),
        }
    }

    /// The source/action node this node belongs to (itself for task nodes).
    pub fn task_node(&self) -> Option<NodeId> {
        match self {
            NodeId::Source(_) | NodeId::Action(_) => Some(self.clone()),
            NodeId::SourceOpt { source, .. } => Some(NodeId::Source(source.clone())),
            NodeId::ActionOpt { action, .. } => Some(NodeId::Action(action.clone())),
            NodeId::Group(_) | NodeId::Category(_) => None,
        }
    }

    /// The option name, for option nodes.
    pub fn opt_name(&self) -> Option<&str> {
        match self {
            NodeId::SourceOpt { opt, .. } | NodeId::ActionOpt { opt, .. } => Some(opt),
            _ => None,
        }
    }

    /// The option of this node's task that is filled from the IFC picker,
    /// and which of the picked file's paths it takes.
    pub fn picker_option(&self) -> Option<(&'static str, PickedFile)> {
        match self.task_node()? {
            NodeId::Source(name) if name == "ifc" => Some(("file", PickedFile::Ifc)),
            NodeId::Action(name) if name == "wall-orientation-wwr" => {
                Some(("input", PickedFile::Json))
            }
            NodeId::Action(name) if name == "murb-simulate" => Some(("model", PickedFile::Json)),
            _ => None,
        }
    }

    /// The option naming the results file the TUI re-reads once this
    /// node's task succeeds.
    pub fn results_option(&self) -> Option<&'static str> {
        match self.task_node()? {
            NodeId::Action(name) if name == "murb-simulate" => Some("output"),
            _ => None,
        }
    }

    /// The source/action this node executes, if any.
    pub fn task_name(&self) -> Option<&str> {
        match self {
            NodeId::Source(name) | NodeId::Action(name) => Some(name),
            NodeId::SourceOpt { source, .. } => Some(source),
            NodeId::ActionOpt { action, .. } => Some(action),
            NodeId::Group(_) | NodeId::Category(_) => None,
        }
    }

    /// Whether pressing execute on this node runs a task.
    pub fn is_executable(&self) -> bool {
        self.task_name().is_some()
    }

    /// The name used for context routing (preview kind, IFC picker):
    /// the task name for task/option nodes, otherwise the node name.
    fn routing_name(&self) -> &str {
        self.task_name().unwrap_or_else(|| self.display_name())
    }

    /// Whether the IFC file picker applies to this node: its task takes one
    /// of its options from the picked file.
    pub fn is_ifc_context(&self) -> bool {
        self.picker_option().is_some()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeId::Group(Group::Sources) => write!(f, "sources"),
            NodeId::Group(Group::Actions) => write!(f, "actions"),
            NodeId::Category(name) => write!(f, "cat:{name}"),
            NodeId::Source(name) => write!(f, "source:{name}"),
            NodeId::SourceOpt { source, opt } => write!(f, "source:{source}:opt:{opt}"),
            NodeId::Action(name) => write!(f, "action:{name}"),
            NodeId::ActionOpt { action, opt } => write!(f, "action:{action}:opt:{opt}"),
        }
    }
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// Which braille preview the detail pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    /// Monthly heating-demand bars (MURB).
    Energy,
    /// Building envelope with wall orientation arrows.
    Walls,
    /// Isometric wireframe: the picked IFC model's walls on IFC nodes, else
    /// an illustration.
    Building,
    /// Floor plan with rooms.
    FloorPlan,
    /// Animated makit logo.
    Logo,
}

impl PreviewKind {
    /// Pick the preview for a node, by its task name:
    /// murb → wall → floor|room → revit|architect → logo.
    pub fn for_node(node: Option<&NodeId>) -> Self {
        let Some(node) = node else {
            return PreviewKind::Logo;
        };
        let name = node.routing_name();
        if name.contains("murb") {
            PreviewKind::Energy
        } else if name.contains("wall") {
            PreviewKind::Walls
        } else if name.contains("floor") || name.contains("room") {
            PreviewKind::FloorPlan
        } else if name.contains("revit") || name.contains("architect") {
            PreviewKind::Building
        } else {
            PreviewKind::Logo
        }
    }

    /// Whether this preview animates (needs ticks).
    pub fn is_animated(self) -> bool {
        self == PreviewKind::Logo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_id_shape() {
        assert_eq!(
            NodeId::parse("sources"),
            Some(NodeId::Group(Group::Sources))
        );
        assert_eq!(
            NodeId::parse("actions"),
            Some(NodeId::Group(Group::Actions))
        );
        assert_eq!(
            NodeId::parse("cat:analysis"),
            Some(NodeId::Category("analysis".into()))
        );
        assert_eq!(
            NodeId::parse("source:ifc"),
            Some(NodeId::Source("ifc".into()))
        );
        assert_eq!(
            NodeId::parse("source:ifc:opt:path"),
            Some(NodeId::SourceOpt {
                source: "ifc".into(),
                opt: "path".into()
            })
        );
        assert_eq!(
            NodeId::parse("action:murb-simulate"),
            Some(NodeId::Action("murb-simulate".into()))
        );
        assert_eq!(
            NodeId::parse("action:murb-simulate:opt:climate"),
            Some(NodeId::ActionOpt {
                action: "murb-simulate".into(),
                opt: "climate".into()
            })
        );
    }

    #[test]
    fn rejects_malformed_ids() {
        for id in [
            "",
            "cat:",
            "source:",
            "action:",
            "source::opt:x",
            "action:a:opt:",
            "nope",
        ] {
            assert_eq!(NodeId::parse(id), None, "{id:?} should not parse");
        }
    }

    #[test]
    fn display_round_trips() {
        for id in [
            "sources",
            "actions",
            "cat:analysis",
            "source:ifc",
            "source:ifc:opt:path",
            "action:revit-extract-walls",
            "action:revit-extract-walls:opt:level",
        ] {
            assert_eq!(NodeId::parse(id).unwrap().to_string(), id);
        }
    }

    #[test]
    fn names_and_executability() {
        let opt = NodeId::parse("action:murb-simulate:opt:climate").unwrap();
        assert_eq!(opt.display_name(), "climate");
        assert_eq!(opt.task_name(), Some("murb-simulate"));
        assert!(opt.is_executable());

        let cat = NodeId::parse("cat:analysis").unwrap();
        assert_eq!(cat.display_name(), "analysis");
        assert!(!cat.is_executable());
        assert!(!NodeId::Group(Group::Sources).is_executable());
    }

    #[test]
    fn preview_routing_priority() {
        let kind = |id: &str| PreviewKind::for_node(NodeId::parse(id).as_ref());
        assert_eq!(kind("action:murb-simulate"), PreviewKind::Energy);
        assert_eq!(kind("action:murb-simulate:opt:wall_r"), PreviewKind::Energy);
        // "revit-extract-walls" contains both "revit" and "wall": wall wins.
        assert_eq!(kind("action:revit-extract-walls"), PreviewKind::Walls);
        assert_eq!(kind("action:wall-orientation-wwr"), PreviewKind::Walls);
        // Floor/room tasks get the floor plan even though they are Revit tasks.
        assert_eq!(kind("action:revit-extract-floors"), PreviewKind::FloorPlan);
        assert_eq!(kind("action:revit-extract-rooms"), PreviewKind::FloorPlan);
        assert_eq!(kind("action:floor-plan"), PreviewKind::FloorPlan);
        assert_eq!(kind("action:room-schedule"), PreviewKind::FloorPlan);
        assert_eq!(kind("source:revit"), PreviewKind::Building);
        assert_eq!(kind("action:revit-calculate-areas"), PreviewKind::Building);
        assert_eq!(kind("action:architect-render-demo"), PreviewKind::Building);
        assert_eq!(kind("source:ifc"), PreviewKind::Logo);
        assert_eq!(kind("sources"), PreviewKind::Logo);
        assert_eq!(PreviewKind::for_node(None), PreviewKind::Logo);
    }

    #[test]
    fn option_names_do_not_change_routing() {
        // An option called "wall_type" under the ifc source must not route to Walls.
        let kind = PreviewKind::for_node(NodeId::parse("source:ifc:opt:wall_type").as_ref());
        assert_eq!(kind, PreviewKind::Logo);
    }

    #[test]
    fn ifc_context_detection() {
        let ctx = |id: &str| NodeId::parse(id).unwrap().is_ifc_context();
        assert!(ctx("source:ifc"));
        assert!(ctx("source:ifc:opt:file"));
        // The WWR action reads the picked file's JSON extraction.
        assert!(ctx("action:wall-orientation-wwr"));
        assert!(ctx("action:wall-orientation-wwr:opt:unit"));
        assert!(ctx("action:murb-simulate"));
        assert!(!ctx("action:revit-wall-orientations"));
        assert!(!ctx("source:revit"));
        assert!(!ctx("cat:ifc"));
        assert!(!ctx("actions"));
    }

    #[test]
    fn task_node_and_option_name() {
        let opt = NodeId::parse("source:ifc:opt:file").unwrap();
        assert_eq!(opt.task_node(), Some(NodeId::Source("ifc".into())));
        assert_eq!(opt.opt_name(), Some("file"));

        let action = NodeId::parse("action:murb-simulate").unwrap();
        assert_eq!(action.task_node(), Some(action.clone()));
        assert_eq!(action.opt_name(), None);

        assert_eq!(NodeId::parse("cat:analysis").unwrap().task_node(), None);
        assert_eq!(NodeId::Group(Group::Actions).task_node(), None);
    }

    #[test]
    fn labels_name_the_task_for_options() {
        let label = |id: &str| NodeId::parse(id).unwrap().label();
        assert_eq!(
            label("action:murb-simulate:opt:output"),
            "murb-simulate --output"
        );
        assert_eq!(label("source:revit:opt:output"), "revit --output");
        assert_eq!(label("action:murb-simulate"), "murb-simulate");
        assert_eq!(label("cat:analysis"), "analysis");
        assert_eq!(label("sources"), "sources");
    }

    #[test]
    fn results_binding() {
        let binding = |id: &str| NodeId::parse(id).unwrap().results_option();
        assert_eq!(binding("action:murb-simulate"), Some("output"));
        assert_eq!(binding("action:murb-simulate:opt:epw"), Some("output"));
        assert_eq!(binding("action:murb-report"), None);
        assert_eq!(binding("source:ifc"), None);
        assert_eq!(binding("actions"), None);
    }

    #[test]
    fn picker_bindings() {
        let binding = |id: &str| NodeId::parse(id).unwrap().picker_option();
        assert_eq!(binding("source:ifc"), Some(("file", PickedFile::Ifc)));
        assert_eq!(
            binding("action:wall-orientation-wwr"),
            Some(("input", PickedFile::Json))
        );
        // Option nodes resolve through their task.
        assert_eq!(
            binding("source:ifc:opt:file"),
            Some(("file", PickedFile::Ifc))
        );
        // murb-simulate takes its window directions from the picked model.
        assert_eq!(
            binding("action:murb-simulate"),
            Some(("model", PickedFile::Json))
        );
        assert_eq!(binding("sources"), None);
    }
}
