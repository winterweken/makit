//! Standalone analysis actions (geometric, performance, IFC).

use std::path::Path;

use makit_core::registry::Registry;
use std::sync::Arc;

use crate::building_model;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_action(
        "wall-orientation-wwr",
        "Wall orientation + WWR analysis",
        "analysis",
        Arc::new(|ctx| {
            let input = ctx.get_option("input", "");
            if input.is_empty() {
                anyhow::bail!("--input is required: path to a building-model.json file");
            }

            let path = Path::new(&input);
            if !path.exists() {
                anyhow::bail!("File not found: {}", input);
            }

            let model = building_model::load_model(path)?;
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
