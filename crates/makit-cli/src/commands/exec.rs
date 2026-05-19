//! `makit exec` — execute a registered task.

use anyhow::Result;
use makit_core::{Registry, TaskContext};

pub fn run(
    tool: &str,
    category: &str,
    task: &str,
    options: &[(String, String)],
) -> Result<()> {
    let reg = Registry::global();
    let reg = reg.read().unwrap();

    let mut ctx = TaskContext::new();
    ctx.tool = tool.to_string();
    ctx.category = category.to_string();
    ctx.task = task.to_string();
    for (k, v) in options {
        ctx.options.insert(k.clone(), v.clone());
    }

    // First check if it's an action
    if let Some(action) = reg.actions.get(task) {
        println!("Executing action: {} [{}]", action.name, action.category);
        (action.handler)(&ctx)?;
        return Ok(());
    }

    // Then check if it's a tool task
    reg.execute_task(tool, category, task, &ctx)
}
