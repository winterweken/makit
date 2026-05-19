//! Revit source + extraction/analysis actions.

use std::sync::Arc;
use makit_core::registry::Registry;

pub fn register_tasks(reg: &mut Registry) {
    // Register Revit as a Source
    reg.register_source("revit", "Autodesk Revit integration", Arc::new(|_ctx| {
        println!("Extracting building model from Revit...");
        Ok(())
    }))
    .add_option("workset", "Filter by workset name", "string", false, Some(""))
    .add_option("wall-type", "Filter by wall type", "string", false, Some(""))
    .add_option("output", "Output file path", "string", false, Some("building-model.json"));

    // Extraction actions
    reg.register_action("revit-extract-walls", "Extract wall elements from Revit", "extraction", Arc::new(|_ctx| {
        println!("Extracting walls from Revit model...");
        Ok(())
    }))
    .add_option("output", "Output file path", "string", false, Some("walls.json"))
    .add_option("level", "Filter by level name", "string", false, Some(""));

    reg.register_action("revit-extract-floors", "Extract floor elements from Revit", "extraction", Arc::new(|_ctx| {
        println!("Extracting floors from Revit model...");
        Ok(())
    }))
    .add_option("output", "Output file path", "string", false, Some("floors.json"));

    reg.register_action("revit-extract-rooms", "Extract room elements from Revit", "extraction", Arc::new(|_ctx| {
        println!("Extracting rooms from Revit model...");
        Ok(())
    }))
    .add_option("output", "Output file path", "string", false, Some("rooms.json"));

    // Analysis actions
    reg.register_action("revit-wall-orientations", "Analyze wall orientations in Revit", "analysis", Arc::new(|_ctx| {
        println!("Analyzing wall orientations...");
        Ok(())
    }))
    .add_option("workset", "Filter by workset name", "string", false, Some(""))
    .add_option("wall-type", "Filter by wall type", "string", false, Some(""))
    .add_option("unit", "Area unit (sqm, sqf)", "string", false, Some("sqm"))
    .add_option("output", "Save results to file", "string", false, Some(""));

    reg.register_action("revit-calculate-areas", "Calculate areas of rooms/spaces", "analysis", Arc::new(|_ctx| {
        println!("Calculating areas...");
        Ok(())
    }))
    .add_option("unit", "Area unit (sqft, sqm)", "string", false, Some("sqft"));

    reg.register_action("revit-find-clashes", "Detect clashes in Revit", "analysis", Arc::new(|_ctx| {
        println!("Finding clashes...");
        Ok(())
    }))
    .add_option("tolerance", "Clash detection tolerance", "float", false, Some("0.01"));

    reg.register_action("revit-validate-standards", "Validate model against standards", "analysis", Arc::new(|_ctx| {
        println!("Validating standards...");
        Ok(())
    }))
    .add_option("ruleset", "Path to validation ruleset", "string", true, None);
}
