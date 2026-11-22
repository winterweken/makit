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

    /// Whether the IFC file picker applies to this node.
    pub fn is_ifc_context(&self) -> bool {
        self.routing_name().contains("ifc")
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
    /// Isometric wireframe building.
    Building,
    /// Floor plan with rooms.
    FloorPlan,
    /// Animated makit logo.
    Logo,
}

impl PreviewKind {
    /// Pick the preview for a node. Order matches the original TUI:
    /// murb → wall → revit|architect → floor|room → logo.
    pub fn for_node(node: Option<&NodeId>) -> Self {
        let Some(node) = node else {
            return PreviewKind::Logo;
        };
        let name = node.routing_name();
        if name.contains("murb") {
            PreviewKind::Energy
        } else if name.contains("wall") {
            PreviewKind::Walls
        } else if name.contains("revit") || name.contains("architect") {
            PreviewKind::Building
        } else if name.contains("floor") || name.contains("room") {
            PreviewKind::FloorPlan
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
    fn preview_routing_keeps_original_priority() {
        let kind = |id: &str| PreviewKind::for_node(NodeId::parse(id).as_ref());
        assert_eq!(kind("action:murb-simulate"), PreviewKind::Energy);
        assert_eq!(kind("action:murb-simulate:opt:wall_r"), PreviewKind::Energy);
        // "revit-extract-walls" contains both "revit" and "wall": wall wins.
        assert_eq!(kind("action:revit-extract-walls"), PreviewKind::Walls);
        assert_eq!(kind("action:wall-orientation-wwr"), PreviewKind::Walls);
        assert_eq!(kind("source:revit"), PreviewKind::Building);
        assert_eq!(kind("action:architect-render-demo"), PreviewKind::Building);
        // "revit" outranks "floor".
        assert_eq!(kind("action:revit-extract-floors"), PreviewKind::Building);
        assert_eq!(kind("action:floor-plan"), PreviewKind::FloorPlan);
        assert_eq!(kind("action:room-schedule"), PreviewKind::FloorPlan);
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
        assert!(NodeId::parse("source:ifc").unwrap().is_ifc_context());
        assert!(NodeId::parse("source:ifc:opt:path")
            .unwrap()
            .is_ifc_context());
        assert!(!NodeId::parse("source:revit").unwrap().is_ifc_context());
        assert!(!NodeId::parse("actions").unwrap().is_ifc_context());
    }
}
