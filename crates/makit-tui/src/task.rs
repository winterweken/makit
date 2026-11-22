//! Resolve an explorer node into a runnable task.

use anyhow::{anyhow, Result};
use makit_core::{Registry, TaskContext, TaskHandler, TaskOption};

use crate::node::NodeId;

/// A task ready to run on a worker thread.
pub struct ResolvedTask {
    /// Source or action name (for status messages).
    pub name: String,
    pub handler: TaskHandler,
    pub ctx: TaskContext,
}

impl std::fmt::Debug for ResolvedTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedTask")
            .field("name", &self.name)
            .field("ctx", &self.ctx)
            .finish_non_exhaustive()
    }
}

/// Look up the handler for `node` and build its context.
///
/// Every option with a default is filled in. For option nodes the selected
/// option is also set — to its default, or `"true"` for flag-like options.
pub fn resolve_task(reg: &Registry, node: &NodeId) -> Result<ResolvedTask> {
    match node {
        NodeId::Source(name) | NodeId::SourceOpt { source: name, .. } => {
            let source = reg
                .sources
                .get(name)
                .ok_or_else(|| anyhow!("unknown source '{name}'"))?;
            Ok(build(name, &source.options, &source.handler, opt_of(node)))
        }
        NodeId::Action(name) | NodeId::ActionOpt { action: name, .. } => {
            let action = reg
                .actions
                .get(name)
                .ok_or_else(|| anyhow!("unknown action '{name}'"))?;
            Ok(build(name, &action.options, &action.handler, opt_of(node)))
        }
        NodeId::Group(_) | NodeId::Category(_) => Err(anyhow!(
            "'{}' is not executable — select a source or action",
            node.display_name()
        )),
    }
}

fn opt_of(node: &NodeId) -> Option<&str> {
    match node {
        NodeId::SourceOpt { opt, .. } | NodeId::ActionOpt { opt, .. } => Some(opt),
        _ => None,
    }
}

fn build(
    name: &str,
    options: &[TaskOption],
    handler: &TaskHandler,
    selected_opt: Option<&str>,
) -> ResolvedTask {
    let mut ctx = TaskContext::new();
    ctx.task = name.to_string();
    for opt in options {
        if let Some(default) = &opt.default {
            ctx.options.insert(opt.name.clone(), default.clone());
        }
    }
    if let Some(opt_name) = selected_opt {
        let value = options
            .iter()
            .find(|o| o.name == opt_name)
            .and_then(|o| o.default.clone())
            .unwrap_or_else(|| "true".to_string());
        ctx.options.insert(opt_name.to_string(), value);
    }
    ResolvedTask {
        name: name.to_string(),
        handler: handler.clone(),
        ctx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn registry(calls: Arc<AtomicUsize>) -> Registry {
        let mut reg = Registry::default();
        let handler: TaskHandler = Arc::new(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        reg.register_source("ifc", "IFC", handler.clone())
            .add_option("path", "Path", "string", true, None)
            .add_option("units", "Units", "string", false, Some("m"));
        reg.register_action("murb-simulate", "MURB", "energy", handler)
            .add_option("climate", "Climate", "string", false, Some("5"))
            .add_option("verbose", "Verbose", "bool", false, None);
        reg
    }

    #[test]
    fn source_gets_defaults() {
        let reg = registry(Arc::default());
        let task = resolve_task(&reg, &NodeId::Source("ifc".into())).unwrap();
        assert_eq!(task.name, "ifc");
        assert_eq!(task.ctx.options.get("units").map(String::as_str), Some("m"));
        assert!(!task.ctx.options.contains_key("path"));
    }

    #[test]
    fn option_node_sets_flag_or_default() {
        let reg = registry(Arc::default());
        let flag = NodeId::ActionOpt {
            action: "murb-simulate".into(),
            opt: "verbose".into(),
        };
        let task = resolve_task(&reg, &flag).unwrap();
        assert_eq!(
            task.ctx.options.get("verbose").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            task.ctx.options.get("climate").map(String::as_str),
            Some("5")
        );

        let with_default = NodeId::ActionOpt {
            action: "murb-simulate".into(),
            opt: "climate".into(),
        };
        let task = resolve_task(&reg, &with_default).unwrap();
        assert_eq!(
            task.ctx.options.get("climate").map(String::as_str),
            Some("5")
        );
    }

    #[test]
    fn resolved_handler_is_the_registered_one() {
        let calls = Arc::new(AtomicUsize::new(0));
        let reg = registry(calls.clone());
        let task = resolve_task(&reg, &NodeId::Action("murb-simulate".into())).unwrap();
        (task.handler)(&task.ctx).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unknown_and_non_executable_nodes_error() {
        let reg = registry(Arc::default());
        let err = resolve_task(&reg, &NodeId::Action("nope".into())).unwrap_err();
        assert!(err.to_string().contains("unknown action 'nope'"));
        let err = resolve_task(&reg, &NodeId::Category("energy".into())).unwrap_err();
        assert!(err.to_string().contains("not executable"));
    }
}
