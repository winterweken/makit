//! Task and tool type definitions for the makit registry.

use std::collections::HashMap;
use std::sync::Arc;

/// Handler function for executing a task.
pub type TaskHandler = Arc<dyn Fn(&TaskContext) -> anyhow::Result<()> + Send + Sync>;

/// Execution context passed to task handlers.
#[derive(Debug, Clone)]
pub struct TaskContext {
    pub tool: String,
    pub category: String,
    pub task: String,
    pub options: HashMap<String, String>,
    pub args: Vec<String>,
}

impl TaskContext {
    pub fn new() -> Self {
        Self {
            tool: String::new(),
            category: String::new(),
            task: String::new(),
            options: HashMap::new(),
            args: Vec::new(),
        }
    }

    /// Get an option value, falling back to a default.
    pub fn get_option(&self, key: &str, default: &str) -> String {
        self.options
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }
}

impl Default for TaskContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Configurable option for a task.
#[derive(Debug, Clone)]
pub struct TaskOption {
    pub name: String,
    pub description: String,
    pub opt_type: String,
    pub required: bool,
    pub default: Option<String>,
}

/// A top-level tool (e.g., Revit, Rhino, Analysis).
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub categories: HashMap<String, Category>,
}

impl Tool {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            categories: HashMap::new(),
        }
    }

    pub fn add_category(&mut self, name: &str, description: &str) -> &mut Category {
        self.categories
            .entry(name.to_string())
            .or_insert_with(|| Category {
                name: name.to_string(),
                description: description.to_string(),
                tasks: HashMap::new(),
            })
    }
}

/// A category of tasks within a tool.
#[derive(Clone)]
pub struct Category {
    pub name: String,
    pub description: String,
    pub tasks: HashMap<String, Task>,
}

impl Category {
    pub fn add_task(&mut self, name: &str, description: &str, handler: TaskHandler) -> &mut Task {
        self.tasks.entry(name.to_string()).or_insert_with(|| Task {
            name: name.to_string(),
            description: description.to_string(),
            handler,
            options: Vec::new(),
        })
    }
}

/// A specific task that can be executed.
#[derive(Clone)]
pub struct Task {
    pub name: String,
    pub description: String,
    pub handler: TaskHandler,
    pub options: Vec<TaskOption>,
}

impl Task {
    pub fn add_option(
        &mut self,
        name: &str,
        description: &str,
        opt_type: &str,
        required: bool,
        default: Option<&str>,
    ) -> &mut Self {
        self.options.push(TaskOption {
            name: name.to_string(),
            description: description.to_string(),
            opt_type: opt_type.to_string(),
            required,
            default: default.map(|s| s.to_string()),
        });
        self
    }

    pub fn execute(&self, ctx: &TaskContext) -> anyhow::Result<()> {
        (self.handler)(ctx)
    }
}

/// A geometry input driver (e.g., Blender, Revit, IFC).
#[derive(Clone)]
pub struct Source {
    pub name: String,
    pub description: String,
    pub handler: TaskHandler,
    pub options: Vec<TaskOption>,
}

impl Source {
    pub fn add_option(
        &mut self,
        name: &str,
        description: &str,
        opt_type: &str,
        required: bool,
        default: Option<&str>,
    ) -> &mut Self {
        self.options.push(TaskOption {
            name: name.to_string(),
            description: description.to_string(),
            opt_type: opt_type.to_string(),
            required,
            default: default.map(|s| s.to_string()),
        });
        self
    }
}

/// An operation performed on geometry (e.g., Analysis, Export).
#[derive(Clone)]
pub struct Action {
    pub name: String,
    pub description: String,
    pub category: String,
    pub handler: TaskHandler,
    pub options: Vec<TaskOption>,
}

impl Action {
    pub fn add_option(
        &mut self,
        name: &str,
        description: &str,
        opt_type: &str,
        required: bool,
        default: Option<&str>,
    ) -> &mut Self {
        self.options.push(TaskOption {
            name: name.to_string(),
            description: description.to_string(),
            opt_type: opt_type.to_string(),
            required,
            default: default.map(|s| s.to_string()),
        });
        self
    }
}
