//! Revit source + extraction/analysis actions.
//!
//! Uses the pyRevit HTTP bridge (port 48884) to extract building geometry
//! and run analysis from the CLI.

pub mod client;
pub mod models;

use std::sync::Arc;

use makit_core::models::TaskContext;
use makit_core::registry::Registry;

use models::analyze_orientations;

/// Default pyRevit API port.
const REVIT_PORT: u16 = 48884;

pub fn register_tasks(reg: &mut Registry) {
    // Register Revit as a Source
    reg.register_source(
        "revit",
        "Autodesk Revit integration",
        Arc::new(handle_connect),
    )
    .add_option(
        "workset",
        "Filter by workset name",
        "string",
        false,
        Some(""),
    )
    .add_option(
        "wall-type",
        "Filter by wall type",
        "string",
        false,
        Some(""),
    )
    .add_option(
        "output",
        "Output file path",
        "string",
        false,
        Some("building-model.json"),
    );

    // Extraction actions
    reg.register_action(
        "revit-extract-walls",
        "Extract wall elements from Revit",
        "extraction",
        Arc::new(handle_extract_walls),
    )
    .add_option(
        "output",
        "Output file path",
        "string",
        false,
        Some("walls.json"),
    )
    .add_option("level", "Filter by level name", "string", false, Some(""));

    reg.register_action(
        "revit-extract-floors",
        "Extract floor elements from Revit",
        "extraction",
        Arc::new(handle_extract_floors),
    )
    .add_option(
        "output",
        "Output file path",
        "string",
        false,
        Some("floors.json"),
    );

    reg.register_action(
        "revit-extract-rooms",
        "Extract room elements from Revit",
        "extraction",
        Arc::new(handle_extract_rooms),
    )
    .add_option(
        "output",
        "Output file path",
        "string",
        false,
        Some("rooms.json"),
    );

    // Analysis actions
    reg.register_action(
        "revit-wall-orientations",
        "Analyze wall orientations in Revit",
        "analysis",
        Arc::new(handle_wall_orientations),
    )
    .add_option(
        "workset",
        "Filter by workset name",
        "string",
        false,
        Some(""),
    )
    .add_option(
        "wall-type",
        "Filter by wall type",
        "string",
        false,
        Some(""),
    )
    .add_option("unit", "Area unit (sqm, sqf)", "string", false, Some("sqm"))
    .add_option("output", "Save results to file", "string", false, Some(""));

    reg.register_action(
        "revit-calculate-areas",
        "Calculate areas of rooms/spaces",
        "analysis",
        Arc::new(handle_calculate_areas),
    )
    .add_option(
        "unit",
        "Area unit (sqft, sqm)",
        "string",
        false,
        Some("sqft"),
    );

    reg.register_action(
        "revit-find-clashes",
        "Detect clashes in Revit",
        "analysis",
        Arc::new(handle_find_clashes),
    )
    .add_option(
        "tolerance",
        "Clash detection tolerance",
        "float",
        false,
        Some("0.01"),
    );

    reg.register_action(
        "revit-validate-standards",
        "Validate model against standards",
        "analysis",
        Arc::new(handle_validate_standards),
    )
    .add_option(
        "ruleset",
        "Path to validation ruleset",
        "string",
        true,
        None,
    );
}

// ---------------------------------------------------------------------------
// Handler Implementations
// ---------------------------------------------------------------------------

/// Check Revit connection status.
fn handle_connect(_ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let connected = rt.block_on(client::check_connection(REVIT_PORT))?;

    if connected {
        println!("✓ Revit connected on port {}", REVIT_PORT);
    } else {
        println!("✗ Revit not connected");
        println!("  Ensure pyRevit extension is loaded and Revit is running");
        println!("  Expected: http://localhost:{}/api/status", REVIT_PORT);
    }
    Ok(())
}

/// Extract walls and save to JSON.
fn handle_extract_walls(ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let walls = rt.block_on(client::extract_walls(REVIT_PORT))?;

    println!("Extracted {} walls", walls.len());

    let output = ctx.get_option("output", "walls.json");
    if !output.is_empty() {
        let json = serde_json::to_string_pretty(&walls)?;
        std::fs::write(&output, json)?;
        println!("Saved to: {}", output);
    }
    Ok(())
}

/// Extract floors and save to JSON.
fn handle_extract_floors(ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let floors = rt.block_on(client::extract_floors(REVIT_PORT))?;

    println!("Extracted {} floors", floors.len());

    let output = ctx.get_option("output", "floors.json");
    if !output.is_empty() {
        let json = serde_json::to_string_pretty(&floors)?;
        std::fs::write(&output, json)?;
        println!("Saved to: {}", output);
    }
    Ok(())
}

/// Extract rooms and save to JSON.
fn handle_extract_rooms(ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let rooms = rt.block_on(client::extract_rooms(REVIT_PORT))?;

    println!("Extracted {} rooms", rooms.len());

    let output = ctx.get_option("output", "rooms.json");
    if !output.is_empty() {
        let json = serde_json::to_string_pretty(&rooms)?;
        std::fs::write(&output, json)?;
        println!("Saved to: {}", output);
    }
    Ok(())
}

/// Analyze wall orientations with area breakdown.
fn handle_wall_orientations(ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let walls = rt.block_on(client::extract_walls(REVIT_PORT))?;

    let results = analyze_orientations(&walls);

    // Print table header
    println!();
    println!("  ╔═══════════╦═══════╦══════════════╦═════════╗");
    println!("  ║ Direction ║ Count ║ Area (m²)    ║ % Total ║");
    println!("  ╠═══════════╬═══════╬══════════════╬═════════╣");

    for r in &results {
        println!(
            "  ║ {:<9} ║ {:>5} ║ {:>12.1} ║ {:>6.1}% ║",
            r.direction, r.count, r.total_area_sqm, r.percentage
        );
    }
    println!("  ╚═══════════╩═══════╩══════════════╩═════════╝");

    let total: f64 = results.iter().map(|r| r.total_area_sqm).sum();
    let total_count: usize = results.iter().map(|r| r.count).sum();
    println!("  Total: {} walls, {:.1} m²", total_count, total);

    // Save to file if requested
    let output = ctx.get_option("output", "");
    if !output.is_empty() {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output, json)?;
        println!("\n  Results saved to: {}", output);
    }

    Ok(())
}

/// Calculate room/space areas.
fn handle_calculate_areas(ctx: &TaskContext) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let rooms = rt.block_on(client::extract_rooms(REVIT_PORT))?;
    let unit = ctx.get_option("unit", "sqft");

    println!();
    println!("  Room Areas:");
    println!("  ──────────────────────────────────");

    for room in &rooms {
        let area = if unit == "sqft" {
            room.area_sqm * 10.7639 // Convert m² to sqft
        } else {
            room.area_sqm
        };
        let unit_label = if unit == "sqft" { "ft²" } else { "m²" };
        println!(
            "  {} ({}): {:.1} {}",
            room.name, room.number, area, unit_label
        );
    }

    let total: f64 = rooms.iter().map(|r| r.area_sqm).sum();
    let total_display = if unit == "sqft" {
        total * 10.7639
    } else {
        total
    };
    let unit_label = if unit == "sqft" { "ft²" } else { "m²" };
    println!("  ──────────────────────────────────");
    println!("  Total: {:.1} {}", total_display, unit_label);
    Ok(())
}

/// Clash detection (stub — requires geometry intersection logic).
fn handle_find_clashes(ctx: &TaskContext) -> anyhow::Result<()> {
    let tolerance: f64 = ctx.get_option("tolerance", "0.01").parse()?;
    println!("Clash detection with tolerance {:.3}m", tolerance);
    println!("(Not yet implemented — requires geometry intersection engine)");
    Ok(())
}

/// Standards validation (stub — requires ruleset parser).
fn handle_validate_standards(ctx: &TaskContext) -> anyhow::Result<()> {
    let ruleset = ctx.get_option("ruleset", "");
    if ruleset.is_empty() {
        anyhow::bail!("--ruleset is required: path to validation ruleset");
    }
    println!("Validating model against ruleset: {}", ruleset);
    println!("(Not yet implemented — requires ruleset parser)");
    Ok(())
}
