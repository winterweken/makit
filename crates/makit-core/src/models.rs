//! Task and tool type definitions for the makit registry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Handler function for executing a task.
pub type TaskHandler = Arc<dyn Fn(&TaskContext) -> anyhow::Result<()> + Send + Sync>;

/// A shared flag asking a running task to stop.
///
/// Long-running handlers (servers, simulations) should poll
/// [`CancelToken::is_cancelled`] and return when it is set. Clones share the
/// same flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the task holding this token to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether a stop has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Execution context passed to task handlers.
#[derive(Debug, Clone)]
pub struct TaskContext {
    pub tool: String,
    pub category: String,
    pub task: String,
    pub options: HashMap<String, String>,
    pub args: Vec<String>,
    /// Set by the caller to ask a long-running handler to stop.
    pub cancel: CancelToken,
}

impl TaskContext {
    pub fn new() -> Self {
        Self {
            tool: String::new(),
            category: String::new(),
            task: String::new(),
            options: HashMap::new(),
            args: Vec::new(),
            cancel: CancelToken::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_is_shared_between_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn new_context_is_not_cancelled() {
        let ctx = TaskContext::new();
        assert!(!ctx.cancel.is_cancelled());
        // A clone of the context observes the same token.
        let copy = ctx.clone();
        ctx.cancel.cancel();
        assert!(copy.cancel.is_cancelled());
    }
}
