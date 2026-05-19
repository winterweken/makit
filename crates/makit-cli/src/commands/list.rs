//! `makit list` — display registered tools, sources, and actions.

use anyhow::Result;
use makit_core::Registry;

pub fn run(tool_filter: Option<String>) -> Result<()> {
    let reg = Registry::global();
    let reg = reg.read().unwrap();

    // Print tools
    println!("╔══════════════════════════════════════════════════╗");
    println!("║                   makit tools                   ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // Sources
    println!("  ┌─ Sources ─────────────────────────────────────┐");
    for source in reg.list_sources() {
        println!("  │  ◆ {:<16} {}", source.name, source.description);
        if !source.options.is_empty() {
            for opt in &source.options {
                let req = if opt.required { " *" } else { "" };
                let def = opt.default.as_deref().unwrap_or("");
                println!(
                    "  │    └ --{:<14} {}{} {}",
                    opt.name,
                    opt.description,
                    req,
                    if def.is_empty() {
                        String::new()
                    } else {
                        format!("[default: {def}]")
                    }
                );
            }
        }
    }
    println!("  └──────────────────────────────────────────────┘");
    println!();

    // Actions
    println!("  ┌─ Actions ─────────────────────────────────────┐");
    for action in reg.list_actions() {
        if let Some(ref filter) = tool_filter {
            if !action.name.starts_with(filter) {
                continue;
            }
        }
        println!("  │  ▸ {:<28} [{}]", action.name, action.category);
        println!("  │    {}", action.description);
        if !action.options.is_empty() {
            for opt in &action.options {
                let req = if opt.required { " *" } else { "" };
                let def = opt.default.as_deref().unwrap_or("");
                println!(
                    "  │    └ --{:<14} {}{} {}",
                    opt.name,
                    opt.description,
                    req,
                    if def.is_empty() {
                        String::new()
                    } else {
                        format!("[default: {def}]")
                    }
                );
            }
        }
    }
    println!("  └──────────────────────────────────────────────┘");

    Ok(())
}
