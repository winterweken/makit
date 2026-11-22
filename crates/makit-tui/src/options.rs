//! Option metadata for sources and actions, copied out of the registry at
//! startup so `update` can reason about options without touching it.

use std::collections::HashMap;

use makit_core::{Registry, TaskOption};

use crate::node::NodeId;

/// One option of a source or action, as registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionSpec {
    pub name: String,
    pub description: String,
    /// Registered type name (`string`, `float`, `int`, `bool`, …).
    pub kind: String,
    pub required: bool,
    pub default: Option<String>,
}

impl OptionSpec {
    /// Whether this is an on/off option, toggled rather than typed.
    pub fn is_flag(&self) -> bool {
        matches!(self.kind.as_str(), "bool" | "boolean" | "flag")
    }

    /// One-line description of type, requirement and default.
    pub fn summary(&self) -> String {
        let need = if self.required {
            "required"
        } else {
            "optional"
        };
        match self.default.as_deref() {
            Some(default) if !default.is_empty() => {
                format!("{} · {need} · default {default}", self.kind)
            }
            _ => format!("{} · {need}", self.kind),
        }
    }
}

impl From<&TaskOption> for OptionSpec {
    fn from(opt: &TaskOption) -> Self {
        Self {
            name: opt.name.clone(),
            description: opt.description.clone(),
            kind: opt.opt_type.clone(),
            required: opt.required,
            default: opt.default.clone(),
        }
    }
}

/// Options of every source and action, keyed by the task's node.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    tasks: HashMap<NodeId, Vec<OptionSpec>>,
}

impl Catalog {
    /// Copy option metadata out of `reg`.
    pub fn from_registry(reg: &Registry) -> Self {
        let mut catalog = Self::default();
        for source in reg.list_sources() {
            catalog.insert(
                NodeId::Source(source.name.clone()),
                source.options.iter().map(OptionSpec::from).collect(),
            );
        }
        for action in reg.list_actions() {
            catalog.insert(
                NodeId::Action(action.name.clone()),
                action.options.iter().map(OptionSpec::from).collect(),
            );
        }
        catalog
    }

    /// Copy option metadata out of the global registry.
    pub fn from_global() -> Self {
        match Registry::global().read() {
            Ok(reg) => Self::from_registry(&reg),
            Err(_) => Self::default(),
        }
    }

    /// Register the options of `task`.
    pub fn insert(&mut self, task: NodeId, options: Vec<OptionSpec>) {
        self.tasks.insert(task, options);
    }

    /// Options of `task`, in registration order (empty if unknown).
    pub fn options(&self, task: &NodeId) -> &[OptionSpec] {
        self.tasks.get(task).map_or(&[], Vec::as_slice)
    }

    /// One option of `task`.
    pub fn spec(&self, task: &NodeId, opt: &str) -> Option<&OptionSpec> {
        self.options(task).iter().find(|o| o.name == opt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makit_core::{Registry, TaskHandler};
    use std::sync::Arc;

    fn registry() -> Registry {
        let mut reg = Registry::default();
        let noop: TaskHandler = Arc::new(|_| Ok(()));
        reg.register_source("ifc", "IFC", noop.clone()).add_option(
            "file",
            "Path to IFC file",
            "string",
            true,
            None,
        );
        reg.register_action("murb-simulate", "MURB", "energy", noop.clone())
            .add_option("epw", "Weather file", "string", true, None)
            .add_option("cop", "Heating COP", "float", false, Some("0.85"))
            .add_option("verbose", "Verbose", "bool", false, None);
        reg.register_action("bare", "No options", "misc", noop);
        reg
    }

    #[test]
    fn catalog_lists_options_per_task_in_registration_order() {
        let catalog = Catalog::from_registry(&registry());
        let opts = catalog.options(&NodeId::Action("murb-simulate".into()));
        let names: Vec<&str> = opts.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, vec!["epw", "cop", "verbose"]);
        assert!(opts[0].required);
        assert_eq!(opts[0].default, None);
        assert_eq!(opts[1].default.as_deref(), Some("0.85"));
        assert_eq!(opts[1].kind, "float");

        let ifc = catalog.options(&NodeId::Source("ifc".into()));
        assert_eq!(ifc.len(), 1);
        assert_eq!(ifc[0].description, "Path to IFC file");
    }

    #[test]
    fn looks_up_one_option() {
        let catalog = Catalog::from_registry(&registry());
        let task = NodeId::Action("murb-simulate".into());
        assert_eq!(
            catalog.spec(&task, "cop").map(|o| o.kind.as_str()),
            Some("float")
        );
        assert!(catalog.spec(&task, "nope").is_none());
    }

    #[test]
    fn unknown_or_optionless_tasks_have_no_options() {
        let catalog = Catalog::from_registry(&registry());
        assert!(catalog.options(&NodeId::Action("bare".into())).is_empty());
        assert!(catalog
            .options(&NodeId::Action("missing".into()))
            .is_empty());
        assert!(catalog
            .options(&NodeId::Category("energy".into()))
            .is_empty());
    }

    #[test]
    fn flags_are_detected_by_type() {
        let catalog = Catalog::from_registry(&registry());
        let task = NodeId::Action("murb-simulate".into());
        assert!(catalog.spec(&task, "verbose").unwrap().is_flag());
        assert!(!catalog.spec(&task, "epw").unwrap().is_flag());
    }

    #[test]
    fn summary_describes_type_requirement_and_default() {
        let catalog = Catalog::from_registry(&registry());
        let task = NodeId::Action("murb-simulate".into());
        assert_eq!(
            catalog.spec(&task, "epw").unwrap().summary(),
            "string · required"
        );
        assert_eq!(
            catalog.spec(&task, "cop").unwrap().summary(),
            "float · optional · default 0.85"
        );
    }
}
