//! Build the tool/action tree for the TUI explorer.

use std::collections::BTreeMap;

use makit_core::Registry;

use crate::tree::TreeNode;

/// Build tree nodes from the global registry.
pub fn build_tree_from_global() -> Vec<TreeNode> {
    let reg = Registry::global();
    let guard = match reg.read() {
        Ok(guard) => guard,
        Err(_) => return Vec::new(),
    };
    build_tree(&guard)
}

/// Build tree nodes from a registry: `Sources` and `Actions` (grouped by
/// category), each with their options as children.
pub fn build_tree(reg: &Registry) -> Vec<TreeNode> {
    let mut sources_node =
        TreeNode::new("sources", "Sources").description("Geometry input drivers");
    for source in reg.list_sources() {
        let mut item = TreeNode::new(
            format!("source:{}", source.name),
            format!("◆ {}", source.name),
        )
        .description(source.description.clone());
        for opt in &source.options {
            item = item.child(
                TreeNode::new(
                    format!("source:{}:opt:{}", source.name, opt.name),
                    format!("--{}: {}", opt.name, opt.description),
                )
                .description(opt.description.clone()),
            );
        }
        sources_node = sources_node.child(item);
    }

    let mut by_category: BTreeMap<&str, Vec<TreeNode>> = BTreeMap::new();
    for action in reg.list_actions() {
        let mut item = TreeNode::new(
            format!("action:{}", action.name),
            format!("▸ {}", action.name),
        )
        .description(action.description.clone());
        for opt in &action.options {
            let req = if opt.required { " *" } else { "" };
            let suffix = match opt.default.as_deref() {
                Some(def) if !def.is_empty() => format!(" [{def}]"),
                _ => String::new(),
            };
            item = item.child(
                TreeNode::new(
                    format!("action:{}:opt:{}", action.name, opt.name),
                    format!("--{}: {}{}{}", opt.name, opt.description, req, suffix),
                )
                .description(opt.description.clone()),
            );
        }
        by_category
            .entry(action.category.as_str())
            .or_default()
            .push(item);
    }

    let mut actions_node =
        TreeNode::new("actions", "Actions").description("Operations on geometry");
    for (cat, items) in by_category {
        let mut cat_node = TreeNode::new(format!("cat:{cat}"), format!("⊞ {cat}"))
            .description(format!("{cat} actions"));
        for item in items {
            cat_node = cat_node.child(item);
        }
        actions_node = actions_node.child(cat_node);
    }

    vec![sources_node, actions_node]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn registry() -> Registry {
        let mut reg = Registry::default();
        let noop: makit_core::TaskHandler = Arc::new(|_| Ok(()));
        reg.register_source("ifc", "IFC file loader", noop.clone())
            .add_option("path", "IFC file path", "string", true, None);
        reg.register_action("murb-simulate", "Run MURB", "energy", noop.clone())
            .add_option("climate", "Climate zone", "string", false, Some("5"));
        reg.register_action("wall-orientation-wwr", "Wall WWR", "analysis", noop);
        reg
    }

    #[test]
    fn builds_sources_and_categorised_actions() {
        let nodes = build_tree(&registry());
        assert_eq!(nodes.len(), 2);
        let sources = &nodes[0];
        assert_eq!(sources.id, "sources");
        assert_eq!(sources.children[0].id, "source:ifc");
        assert_eq!(sources.children[0].label, "◆ ifc");
        assert_eq!(sources.children[0].description, "IFC file loader");
        assert_eq!(sources.children[0].children[0].id, "source:ifc:opt:path");
        assert_eq!(
            sources.children[0].children[0].label,
            "--path: IFC file path"
        );

        let actions = &nodes[1];
        let cats: Vec<&str> = actions.children.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(cats, vec!["cat:analysis", "cat:energy"]); // BTreeMap order
        let murb = &actions.children[1].children[0];
        assert_eq!(murb.id, "action:murb-simulate");
        assert_eq!(murb.children[0].label, "--climate: Climate zone [5]");
    }

    #[test]
    fn required_options_are_starred() {
        let mut reg = Registry::default();
        let noop: makit_core::TaskHandler = Arc::new(|_| Ok(()));
        reg.register_action("a", "A", "x", noop)
            .add_option("level", "Level", "string", true, None);
        let nodes = build_tree(&reg);
        let opt = &nodes[1].children[0].children[0].children[0];
        assert_eq!(opt.label, "--level: Level *");
    }
}
