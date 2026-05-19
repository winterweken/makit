//! Generic building model format — platform-agnostic JSON.
//!
//! This bridges IFC/Revit/Rhino extracted data into the Rust analysis pipeline.
//! The generic format uses `orientation: {x, y, z}` for wall normals,
//! which is converted to the `WallData.normal` field for analysis.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::revit::models::{analyze_orientations, OrientationResult, WallData};

/// Top-level generic building model (platform-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingModel {
    #[serde(default)]
    pub walls: Vec<GenericWall>,
    #[serde(default)]
    pub windows: Vec<GenericWindow>,
    #[serde(default, rename = "projectNorth")]
    pub project_north: f64,
    #[serde(default)]
    pub units: String,
    #[serde(default)]
    pub metadata: Option<ModelMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericWall {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub orientation: Vec3,
    #[serde(default)]
    pub area: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub length: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default, rename = "type")]
    pub wall_type: String,
    #[serde(default)]
    pub workset: String,
    #[serde(default, rename = "isCurtainWall")]
    pub is_curtain_wall: bool,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub windows: Vec<WallWindow>,
    #[serde(default)]
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallWindow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub area: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericWindow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub area: f64,
    #[serde(default, rename = "hostWallId")]
    pub host_wall_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    #[serde(default, rename = "projectName")]
    pub project_name: String,
    #[serde(default)]
    pub description: String,
}

// ---------------------------------------------------------------------------
// Analysis results
// ---------------------------------------------------------------------------

/// Complete WWR analysis result.
#[derive(Debug, Clone, Serialize)]
pub struct WwrAnalysis {
    pub orientations: Vec<OrientationResult>,
    pub total_wall_area: f64,
    pub total_window_area: f64,
    pub overall_wwr: f64,
    pub wall_count: usize,
    pub window_count: usize,
    pub project_name: String,
}

// ---------------------------------------------------------------------------
// Loading and conversion
// ---------------------------------------------------------------------------

/// Load a building model from a JSON file.
pub fn load_model(path: &Path) -> anyhow::Result<BuildingModel> {
    let json = std::fs::read_to_string(path)?;
    let model: BuildingModel = serde_json::from_str(&json)?;
    Ok(model)
}

/// Try to find a `.json` companion for an `.ifc` file.
///
/// Checks for `building-model.json` in the same directory,
/// then `<filename>.json`.
pub fn find_json_for_ifc(ifc_path: &Path) -> Option<std::path::PathBuf> {
    let parent = ifc_path.parent().unwrap_or(Path::new("."));

    // Check for building-model.json in the same directory
    let companion = parent.join("building-model.json");
    if companion.exists() {
        return Some(companion);
    }

    // Check for <name>.json
    if let Some(stem) = ifc_path.file_stem() {
        let json_name = format!("{}.json", stem.to_string_lossy());
        let companion = parent.join(json_name);
        if companion.exists() {
            return Some(companion);
        }
    }

    None
}

/// Convert generic walls to the WallData format used by analyze_orientations.
fn to_wall_data(walls: &[GenericWall]) -> Vec<WallData> {
    walls
        .iter()
        .enumerate()
        .map(|(i, w)| WallData {
            id: i as i64,
            wall_type: w.wall_type.clone(),
            level: w.level.clone(),
            area_sqm: w.area,
            length_m: w.length / 1000.0, // generic format stores mm
            height_m: w.height,
            start_point: [0.0, 0.0, 0.0],
            end_point: [w.length / 1000.0, 0.0, 0.0],
            normal: [w.orientation.x, w.orientation.y, w.orientation.z],
        })
        .collect()
}

/// Run full WWR analysis on a building model.
pub fn analyze_wwr(model: &BuildingModel) -> WwrAnalysis {
    let wall_data = to_wall_data(&model.walls);
    let orientations = analyze_orientations(&wall_data);

    let total_wall_area: f64 = model.walls.iter().map(|w| w.area).sum();

    // Window area: from wall-embedded windows + top-level windows
    let wall_window_area: f64 = model
        .walls
        .iter()
        .flat_map(|w| &w.windows)
        .map(|win| win.area)
        .sum();
    let top_window_area: f64 = model.windows.iter().map(|w| w.area).sum();
    let total_window_area = wall_window_area + top_window_area;

    let overall_wwr = if total_wall_area > 0.0 {
        (total_window_area / total_wall_area) * 100.0
    } else {
        0.0
    };

    let window_count =
        model.walls.iter().map(|w| w.windows.len()).sum::<usize>() + model.windows.len();

    let project_name = model
        .metadata
        .as_ref()
        .map(|m| m.project_name.clone())
        .unwrap_or_default();

    WwrAnalysis {
        orientations,
        total_wall_area: ((total_wall_area * 10.0).round() / 10.0).abs(),
        total_window_area: ((total_window_area * 10.0).round() / 10.0).abs(),
        overall_wwr: ((overall_wwr * 10.0).round() / 10.0).abs(),
        wall_count: model.walls.len(),
        window_count,
        project_name,
    }
}

/// Print a formatted WWR analysis report to stdout.
pub fn print_wwr_report(analysis: &WwrAnalysis) {
    if !analysis.project_name.is_empty() {
        println!("\n  Project: {}", analysis.project_name);
    }

    println!();
    println!("  ╔═══════════╦═══════╦══════════════╦═════════╗");
    println!("  ║ Direction ║ Count ║ Area (m²)    ║ % Total ║");
    println!("  ╠═══════════╬═══════╬══════════════╬═════════╣");

    for r in &analysis.orientations {
        println!(
            "  ║ {:<9} ║ {:>5} ║ {:>12.1} ║ {:>6.1}% ║",
            r.direction, r.count, r.total_area_sqm, r.percentage
        );
    }
    println!("  ╚═══════════╩═══════╩══════════════╩═════════╝");
    println!(
        "  Total: {} walls, {:.1} m²",
        analysis.wall_count, analysis.total_wall_area
    );
    println!(
        "  Windows: {}, {:.1} m²",
        analysis.window_count, analysis.total_window_area
    );
    println!("  Overall WWR: {:.1}%", analysis.overall_wwr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_building_model() {
        let path = Path::new("../../examples/IFC/building-model.json");
        if !path.exists() {
            return; // Skip if not in workspace root
        }
        let model = load_model(path).unwrap();
        assert_eq!(model.walls.len(), 4);
        assert_eq!(model.units, "m");
    }

    #[test]
    fn test_analyze_wwr_sample() {
        let path = Path::new("../../examples/IFC/building-model.json");
        if !path.exists() {
            return;
        }
        let model = load_model(path).unwrap();
        let result = analyze_wwr(&model);
        assert_eq!(result.wall_count, 4);
        assert!(result.total_wall_area > 40.0);
        assert_eq!(result.overall_wwr, 0.0); // No windows in sample
        assert!(!result.orientations.is_empty());
    }

    #[test]
    fn test_find_json_for_ifc() {
        let ifc_path = Path::new("../../examples/IFC/Building-Architecture.ifc");
        if !ifc_path.exists() {
            return;
        }
        let json_path = find_json_for_ifc(ifc_path);
        assert!(json_path.is_some());
    }
}
