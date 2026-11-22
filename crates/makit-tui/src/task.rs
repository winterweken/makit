//! Resolve an explorer node into a runnable task.

use std::collections::BTreeMap;

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

/// Look up the handler for `node`'s task and build its context.
///
/// Every option with a registered default is filled in, then `values`
/// (what the user entered or picked) are laid over them. For option nodes
/// the parent task runs; the focused option gets no special treatment.
pub fn resolve_task(
    reg: &Registry,
    node: &NodeId,
    values: &BTreeMap<String, String>,
) -> Result<ResolvedTask> {
    match node {
        NodeId::Source(name) | NodeId::SourceOpt { source: name, .. } => {
            let source = reg
                .sources
                .get(name)
                .ok_or_else(|| anyhow!("unknown source '{name}'"))?;
            Ok(build(name, &source.options, &source.handler, values))
        }
        NodeId::Action(name) | NodeId::ActionOpt { action: name, .. } => {
            let action = reg
                .actions
                .get(name)
                .ok_or_else(|| anyhow!("unknown action '{name}'"))?;
            Ok(build(name, &action.options, &action.handler, values))
        }
        NodeId::Group(_) | NodeId::Category(_) => Err(anyhow!(
            "'{}' is not executable — select a source or action",
            node.display_name()
        )),
    }
}

fn build(
    name: &str,
    options: &[TaskOption],
    handler: &TaskHandler,
    values: &BTreeMap<String, String>,
) -> ResolvedTask {
    let mut ctx = TaskContext::new();
    ctx.task = name.to_string();
    for opt in options {
        if let Some(default) = &opt.default {
            ctx.options.insert(opt.name.clone(), default.clone());
        }
    }
    for (key, value) in values {
        ctx.options.insert(key.clone(), value.clone());
    }
    ResolvedTask {
        name: name.to_string(),
        handler: handler.clone(),
        ctx,
    }
}

/// Replace a leading `~` or `~/` with `home`, as a shell would. Typed paths
/// never pass through a shell, so the TUI does it for the user.
pub fn expand_tilde(value: &str, home: Option<&str>) -> String {
    let Some(home) = home else {
        return value.to_string();
    };
    if value == "~" {
        home.to_string()
    } else if let Some(rest) = value.strip_prefix("~/") {
        format!("{}/{rest}", home.trim_end_matches('/'))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
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

    fn none() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn source_gets_defaults() {
        let reg = registry(Arc::default());
        let task = resolve_task(&reg, &NodeId::Source("ifc".into()), &none()).unwrap();
        assert_eq!(task.name, "ifc");
        assert_eq!(task.ctx.options.get("units").map(String::as_str), Some("m"));
        assert!(!task.ctx.options.contains_key("path"));
    }

    #[test]
    fn option_node_runs_its_task_without_forcing_a_value() {
        let reg = registry(Arc::default());
        let flag = NodeId::ActionOpt {
            action: "murb-simulate".into(),
            opt: "verbose".into(),
        };
        let task = resolve_task(&reg, &flag, &none()).unwrap();
        assert_eq!(task.name, "murb-simulate");
        // The focused option is not silently set to "true".
        assert!(!task.ctx.options.contains_key("verbose"));
        assert_eq!(
            task.ctx.options.get("climate").map(String::as_str),
            Some("5")
        );
    }

    #[test]
    fn given_values_win_over_defaults() {
        let reg = registry(Arc::default());
        let given = values(&[("climate", "7A"), ("verbose", "true")]);
        let task = resolve_task(&reg, &NodeId::Action("murb-simulate".into()), &given).unwrap();
        assert_eq!(
            task.ctx.options.get("climate").map(String::as_str),
            Some("7A")
        );
        assert_eq!(
            task.ctx.options.get("verbose").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = Some("/home/u");
        assert_eq!(expand_tilde("~/models/a.ifc", home), "/home/u/models/a.ifc");
        assert_eq!(expand_tilde("~", home), "/home/u");
        // Only a leading "~" or "~/" is the user's home.
        assert_eq!(expand_tilde("~other/x", home), "~other/x");
        assert_eq!(expand_tilde("/abs/~/x", home), "/abs/~/x");
        assert_eq!(expand_tilde("plain", home), "plain");
        assert_eq!(expand_tilde("~/x", None), "~/x");
    }

    #[test]
    fn resolved_handler_is_the_registered_one() {
        let calls = Arc::new(AtomicUsize::new(0));
        let reg = registry(calls.clone());
        let task = resolve_task(&reg, &NodeId::Action("murb-simulate".into()), &none()).unwrap();
        (task.handler)(&task.ctx).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unknown_and_non_executable_nodes_error() {
        let reg = registry(Arc::default());
        let err = resolve_task(&reg, &NodeId::Action("nope".into()), &none()).unwrap_err();
        assert!(err.to_string().contains("unknown action 'nope'"));
        let err = resolve_task(&reg, &NodeId::Category("energy".into()), &none()).unwrap_err();
        assert!(err.to_string().contains("not executable"));
    }
}
