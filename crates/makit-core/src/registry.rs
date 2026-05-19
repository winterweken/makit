//! Global registry for tools, sources, and actions.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::models::*;

static GLOBAL_REGISTRY: OnceLock<Arc<RwLock<Registry>>> = OnceLock::new();

/// The central registry holding all registered tools, sources, and actions.
#[derive(Default)]
pub struct Registry {
    pub tools: HashMap<String, Tool>,
    pub sources: HashMap<String, Source>,
    pub actions: HashMap<String, Action>,
}

impl Registry {
    /// Get the global registry singleton.
    pub fn global() -> Arc<RwLock<Registry>> {
        GLOBAL_REGISTRY
            .get_or_init(|| Arc::new(RwLock::new(Registry::default())))
            .clone()
    }

    /// Register a new tool.
    pub fn register_tool(&mut self, name: &str, description: &str) -> &mut Tool {
        self.tools
            .entry(name.to_string())
            .or_insert_with(|| Tool::new(name, description))
    }

    /// Register a new geometry source.
    pub fn register_source(
        &mut self,
        name: &str,
        description: &str,
        handler: TaskHandler,
    ) -> &mut Source {
        self.sources
            .entry(name.to_string())
            .or_insert_with(|| Source {
                name: name.to_string(),
                description: description.to_string(),
                handler,
                options: Vec::new(),
            })
    }

    /// Register a new action.
    pub fn register_action(
        &mut self,
        name: &str,
        description: &str,
        category: &str,
        handler: TaskHandler,
    ) -> &mut Action {
        self.actions
            .entry(name.to_string())
            .or_insert_with(|| Action {
                name: name.to_string(),
                description: description.to_string(),
                category: category.to_string(),
                handler,
                options: Vec::new(),
            })
    }

    /// Get a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.get(name)
    }

    /// List all registered tools, sorted by name.
    pub fn list_tools(&self) -> Vec<&Tool> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    /// List all registered sources, sorted by name.
    pub fn list_sources(&self) -> Vec<&Source> {
        let mut sources: Vec<_> = self.sources.values().collect();
        sources.sort_by(|a, b| a.name.cmp(&b.name));
        sources
    }

    /// List all registered actions, sorted by name.
    pub fn list_actions(&self) -> Vec<&Action> {
        let mut actions: Vec<_> = self.actions.values().collect();
        actions.sort_by(|a, b| a.name.cmp(&b.name));
        actions
    }

    /// Get a task by tool, category, and task name.
    pub fn get_task(&self, tool_name: &str, category_name: &str, task_name: &str) -> Option<&Task> {
        self.tools
            .get(tool_name)?
            .categories
            .get(category_name)?
            .tasks
            .get(task_name)
    }

    /// Execute a task with the given context.
    pub fn execute_task(
        &self,
        tool_name: &str,
        category_name: &str,
        task_name: &str,
        ctx: &TaskContext,
    ) -> anyhow::Result<()> {
        let task = self
            .get_task(tool_name, category_name, task_name)
            .ok_or_else(|| {
                anyhow::anyhow!("task '{task_name}' not found in {tool_name}/{category_name}")
            })?;
        task.execute(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_handler() -> TaskHandler {
        Arc::new(|_ctx| Ok(()))
    }

    #[test]
    fn test_register_and_list_tools() {
        let mut reg = Registry::default();
        reg.register_tool("revit", "Autodesk Revit");
        reg.register_tool("rhino", "Rhino 3D");

        let tools = reg.list_tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "revit");
        assert_eq!(tools[1].name, "rhino");
    }

    #[test]
    fn test_register_source_with_options() {
        let mut reg = Registry::default();
        let source = reg.register_source("revit", "Revit integration", noop_handler());
        source.add_option("output", "Output file", "string", false, Some("out.json"));

        assert_eq!(reg.sources["revit"].options.len(), 1);
        assert_eq!(reg.sources["revit"].options[0].name, "output");
    }

    #[test]
    fn test_register_action() {
        let mut reg = Registry::default();
        let action = reg.register_action(
            "wall-orientations",
            "Analyze wall orientations",
            "analysis",
            noop_handler(),
        );
        action.add_option("unit", "Area unit", "string", false, Some("sqm"));

        let actions = reg.list_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].category, "analysis");
    }

    #[test]
    fn test_execute_task() {
        let mut reg = Registry::default();
        let tool = reg.register_tool("test", "Test tool");
        let cat = tool.add_category("cat", "Test category");
        cat.add_task("task1", "Test task", Arc::new(|_| Ok(())));

        let ctx = TaskContext::new();
        let result = reg.execute_task("test", "cat", "task1", &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_missing_task() {
        let reg = Registry::default();
        let ctx = TaskContext::new();
        let result = reg.execute_task("nope", "nope", "nope", &ctx);
        assert!(result.is_err());
    }
}
