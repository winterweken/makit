//! Standalone analysis actions (geometric, performance, IFC).

use std::sync::Arc;
use makit_core::registry::Registry;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_action("wall-orientation-wwr", "Wall orientation + WWR analysis", "analysis", Arc::new(|_ctx| {
        println!("Running wall orientation and WWR analysis...");
        Ok(())
    }))
    .add_option("input", "Input JSON or IFC file", "string", true, None)
    .add_option("unit", "Area unit (sqm, sqf)", "string", false, Some("sqm"))
    .add_option("output", "Output file", "string", false, Some(""));
}
