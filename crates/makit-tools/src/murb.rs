//! MURB energy modelling tool bridge.
//!
//! Bridges the Python murb_energy_tool via JSON subprocess for
//! early-stage TEDI/TEUI/GHGI analysis.

use std::sync::Arc;
use makit_core::registry::Registry;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source("murb", "MURB energy modelling tool", Arc::new(|_ctx| {
        println!("Connecting to MURB energy model...");
        Ok(())
    }));

    reg.register_action("murb-simulate", "Run energy simulation", "analysis", Arc::new(|_ctx| {
        println!("Running MURB energy simulation...");
        // TODO: subprocess call to murb_runner.py with JSON I/O
        Ok(())
    }))
    .add_option("epw", "Path to EPW weather file", "string", true, None)
    .add_option("gfa", "Gross floor area [m²]", "float", true, None)
    .add_option("walls-ag", "Above-grade wall area [m²]", "float", true, None)
    .add_option("u-walls", "Wall U-value [W/m²K]", "float", false, Some("0.273"))
    .add_option("u-windows", "Window U-value [W/m²K]", "float", false, Some("2.56"))
    .add_option("u-roof", "Roof U-value [W/m²K]", "float", false, Some("0.164"))
    .add_option("province", "Canadian province code", "string", false, Some("ON"))
    .add_option("output", "Output JSON path", "string", false, Some("murb_results.json"));

    reg.register_action("murb-report", "Generate energy report", "reporting", Arc::new(|_ctx| {
        println!("Generating MURB energy report...");
        Ok(())
    }))
    .add_option("input", "Simulation results JSON", "string", true, None)
    .add_option("format", "Report format (text, json, html)", "string", false, Some("text"));
}
