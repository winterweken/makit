//! Standalone analysis actions (geometric, performance, IFC).

use std::path::Path;

use makit_core::registry::Registry;
use std::sync::Arc;

use crate::building_model;
use crate::ifc;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_action(
        "wall-orientation-wwr",
        "Wall orientation + WWR analysis",
        "analysis",
        Arc::new(|ctx| {
            let input = ctx.get_option("input", "");
            if input.is_empty() {
                anyhow::bail!("--input is required: path to an IFC file or its extraction");
            }

            let path = Path::new(&input);
            if !path.exists() {
                anyhow::bail!("File not found: {}", input);
            }

            // An IFC file is extracted on demand; its JSON is analysed.
            let json = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ifc"))
            {
                let resolved = ifc::extraction_for(path, Some(&ctx.cancel))?;
                if resolved.extracted {
                    println!("  Extracted to {}", resolved.json.display());
                } else {
                    println!("  Found extracted model: {}", resolved.json.display());
                }
                if let Some(note) = &resolved.note {
                    println!("  {note}");
                }
                resolved.json
            } else {
                path.to_path_buf()
            };

            let model = building_model::load_model(&json)?;
            let analysis = building_model::analyze_wwr(&model);
            building_model::print_wwr_report(&analysis);

            // Save output if requested
            let output = ctx.get_option("output", "");
            if !output.is_empty() {
                let json = serde_json::to_string_pretty(&analysis)?;
                std::fs::write(&output, json)?;
                println!("\n  Results saved to: {}", output);
            }

            Ok(())
        }),
    )
    .add_option("input", "Input JSON or IFC file", "string", true, None)
    .add_option("unit", "Area unit (sqm, sqf)", "string", false, Some("sqm"))
    .add_option("output", "Output file", "string", false, Some(""));
}

#[cfg(test)]
mod tests {
    use super::*;
    use makit_core::models::TaskContext;

    #[test]
    fn wall_orientation_wwr_takes_an_ifc_with_a_fresh_extraction() {
        let mut reg = Registry::default();
        register_tasks(&mut reg);
        let ifc = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/Building-Architecture.ifc"
        );
        let mut ctx = TaskContext::new();
        ctx.options.insert("input".into(), ifc.into());
        (reg.actions["wall-orientation-wwr"].handler)(&ctx).unwrap();

        ctx.options
            .insert("input".into(), "/nowhere/missing.json".into());
        let err = (reg.actions["wall-orientation-wwr"].handler)(&ctx)
            .unwrap_err()
            .to_string();
        assert!(err.contains("File not found"), "{err}");
    }
}
