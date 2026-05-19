//! Build the tool/action tree for the TUI explorer.

use tui::prelude::TreeItem;
use makit_core::Registry;

/// Build the tree items from the current registry state.
pub fn build_tree_items() -> Vec<TreeItem> {
    let reg = Registry::global();
    let reg = reg.read().unwrap();

    let mut source_children = Vec::new();
    for source in reg.list_sources() {
        let mut item = TreeItem::new(
            format!("source:{}", source.name),
            format!("◆ {}", source.name),
        );
        for opt in &source.options {
            item = item.child(TreeItem::new(
                format!("source:{}:opt:{}", source.name, opt.name),
                format!("--{}: {}", opt.name, opt.description),
            ));
        }
        source_children.push(item);
    }

    let mut action_by_cat: std::collections::BTreeMap<String, Vec<TreeItem>> =
        std::collections::BTreeMap::new();
    for action in reg.list_actions() {
        let mut item = TreeItem::new(
            format!("action:{}", action.name),
            format!("▸ {}", action.name),
        );
        for opt in &action.options {
            let req = if opt.required { " *" } else { "" };
            let def = opt.default.as_deref().unwrap_or("");
            let suffix = if def.is_empty() {
                String::new()
            } else {
                format!(" [{}]", def)
            };
            item = item.child(TreeItem::new(
                format!("action:{}:opt:{}", action.name, opt.name),
                format!("--{}: {}{}{}", opt.name, opt.description, req, suffix),
            ));
        }
        action_by_cat
            .entry(action.category.clone())
            .or_default()
            .push(item);
    }

    let mut action_categories = Vec::new();
    for (cat, items) in action_by_cat {
        let mut cat_item = TreeItem::new(
            format!("cat:{}", cat),
            format!("⊞ {}", cat),
        );
        for item in items {
            cat_item = cat_item.child(item);
        }
        action_categories.push(cat_item);
    }

    let mut sources_node = TreeItem::new("sources", "Sources");
    for child in source_children {
        sources_node = sources_node.child(child);
    }

    let mut actions_node = TreeItem::new("actions", "Actions");
    for child in action_categories {
        actions_node = actions_node.child(child);
    }

    vec![sources_node, actions_node]
}
