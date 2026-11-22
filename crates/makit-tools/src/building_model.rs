//! Generic building model format — platform-agnostic JSON.
//!
//! This bridges IFC/Revit/Rhino extracted data into the Rust analysis pipeline.
//! The generic format uses `orientation: {x, y, z}` for wall normals,
//! which is converted to the `WallData.normal` field for analysis.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// `Some(false)` for a wall the source flags as interior; such walls
    /// are left out of the envelope analysis. `None` when not known.
    #[serde(default, rename = "isExternal")]
    pub is_external: Option<bool>,
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
    /// File name of the IFC file this model was extracted from, if the
    /// extractor recorded it.
    #[serde(
        default,
        rename = "sourceFile",
        skip_serializing_if = "String::is_empty"
    )]
    pub source_file: String,
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
    /// Walls flagged interior, left out of every figure above.
    pub interior_walls_left_out: usize,
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

/// What the IFC extractor writes when it is not told where to write.
pub const SHARED_EXTRACTION: &str = "building-model.json";

/// Find the JSON extraction of an `.ifc` file.
///
/// `<name>.json` beside the file is its own. The shared
/// [`SHARED_EXTRACTION`] in the same folder is used only when it belongs to
/// this file: its metadata names this file as `sourceFile`, or it names no
/// source and no other IFC file sits in the folder. Otherwise one model's
/// extraction would be reported for every IFC file beside it.
pub fn find_json_for_ifc(ifc_path: &Path) -> Option<PathBuf> {
    let parent = match ifc_path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let name = ifc_path.file_name()?.to_string_lossy();

    if let Some(stem) = ifc_path.file_stem() {
        let own = parent.join(format!("{}.json", stem.to_string_lossy()));
        if own.exists() {
            return Some(own);
        }
    }

    let shared = parent.join(SHARED_EXTRACTION);
    if !shared.exists() {
        return None;
    }
    let belongs = match extraction_source(&shared) {
        Some(source) => source.eq_ignore_ascii_case(&name),
        None => no_other_ifc_in(parent, &name),
    };
    belongs.then_some(shared)
}

/// Where an `.ifc` file's own extraction belongs: `<name>.json` beside it,
/// the first place [`find_json_for_ifc`] looks.
pub fn extraction_path(ifc_path: &Path) -> PathBuf {
    ifc_path.with_extension("json")
}

/// The shell command that extracts `ifc_path` to [`extraction_path`]. It
/// needs Python with `ifcopenshell` and runs from the repository root.
pub fn extraction_command(ifc_path: &Path) -> String {
    format!(
        "python3 pyrevit-extension/Makit.extension/lib/analyze_ifc.py {} --extract-only --output {}",
        ifc_path.display(),
        extraction_path(ifc_path).display()
    )
}

/// The `sourceFile` an extraction names, if it names one. An extraction
/// that cannot be read names none; loading it later reports why.
fn extraction_source(json: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct Header {
        #[serde(default)]
        metadata: Option<Source>,
    }
    #[derive(Deserialize)]
    struct Source {
        #[serde(default, rename = "sourceFile")]
        source_file: String,
    }
    let text = std::fs::read_to_string(json).ok()?;
    let header: Header = serde_json::from_str(&text).ok()?;
    Some(header.metadata?.source_file).filter(|s| !s.is_empty())
}

/// Whether `dir` holds no IFC file other than `name`. A folder that cannot
/// be listed might, so it counts as holding one.
fn no_other_ifc_in(dir: &Path, name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().all(|entry| {
        let path = entry.path();
        let is_ifc = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ifc"));
        !is_ifc
            || entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
    })
}

/// A wall's outward normal on the true compass (+Y north, +X east).
///
/// The generic format stores the normal in project coordinates with
/// `projectNorth`, the clockwise angle from project to true coordinates, and
/// flags curtain walls, whose normal Revit reports pointing inward. This
/// matches `analysis_engine.calculate_wall_orientation` on the Python side.
fn true_normal(wall: &GenericWall, project_north: f64) -> [f64; 3] {
    let Vec3 { x, y, z } = wall.orientation;
    let (sin, cos) = project_north.sin_cos();
    let (x, y) = (x * cos + y * sin, -x * sin + y * cos);
    if wall.is_curtain_wall {
        [-x, -y, z]
    } else {
        [x, y, z]
    }
}

/// Window area by the way its wall faces on the true compass: pairs of
/// (azimuth in whole degrees clockwise from north, m²), sorted by azimuth.
///
/// Only windows in walls not flagged interior count. Windows outside any
/// wall, such as skylights, have no facing and are left out.
pub fn window_area_by_facing(model: &BuildingModel) -> Vec<(f64, f64)> {
    let mut by_azimuth: std::collections::BTreeMap<i64, f64> = std::collections::BTreeMap::new();
    for wall in model.walls.iter().filter(|w| w.is_external != Some(false)) {
        let area: f64 = wall.windows.iter().map(|w| w.area).sum();
        let [x, y, _] = true_normal(wall, model.project_north);
        if area <= 0.0 || x.hypot(y) < 1e-9 {
            continue;
        }
        let azimuth = (x.atan2(y).to_degrees().round() as i64).rem_euclid(360);
        *by_azimuth.entry(azimuth).or_default() += area;
    }
    by_azimuth
        .into_iter()
        .map(|(azimuth, area)| (azimuth as f64, area))
        .collect()
}

/// Convert generic walls to the WallData format used by analyze_orientations.
fn to_wall_data(walls: &[GenericWall], project_north: f64) -> Vec<WallData> {
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
            normal: true_normal(w, project_north),
        })
        .collect()
}

/// Run full WWR analysis on a building model.
pub fn analyze_wwr(model: &BuildingModel) -> WwrAnalysis {
    // The envelope: every wall not flagged interior. Walls with no flag
    // count, as they did before sources recorded it.
    let (walls, interior): (Vec<GenericWall>, Vec<GenericWall>) = model
        .walls
        .iter()
        .cloned()
        .partition(|w| w.is_external != Some(false));
    let wall_data = to_wall_data(&walls, model.project_north);
    let orientations = analyze_orientations(&wall_data);

    let total_wall_area: f64 = walls.iter().map(|w| w.area).sum();

    // Windows listed in an envelope wall, plus top-level windows not listed
    // in any wall: the IFC extractor puts a hosted window in both lists, and
    // a window in an interior wall is not part of the envelope either.
    let in_walls: Vec<&WallWindow> = walls.iter().flat_map(|w| &w.windows).collect();
    let listed: std::collections::HashSet<&str> = model
        .walls
        .iter()
        .flat_map(|w| &w.windows)
        .map(|win| win.id.as_str())
        .filter(|id| !id.is_empty())
        .collect();
    let loose: Vec<&GenericWindow> = model
        .windows
        .iter()
        .filter(|win| win.id.is_empty() || !listed.contains(win.id.as_str()))
        .collect();
    let total_window_area: f64 = in_walls.iter().map(|win| win.area).sum::<f64>()
        + loose.iter().map(|win| win.area).sum::<f64>();

    // Wall areas are net of their openings, so the glazed share of the
    // facade is window / (wall + window), as in analysis_engine.calculate_wwr.
    let facade = total_wall_area + total_window_area;
    let overall_wwr = if facade > 0.0 {
        (total_window_area / facade) * 100.0
    } else {
        0.0
    };

    let window_count = in_walls.len() + loose.len();

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
        wall_count: walls.len(),
        window_count,
        project_name,
        interior_walls_left_out: interior.len(),
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
    if analysis.interior_walls_left_out > 0 {
        println!(
            "  Interior walls left out: {}",
            analysis.interior_walls_left_out
        );
    }
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
        // The plumbing wall is flagged interior; the three house walls count.
        assert_eq!(result.wall_count, 3);
        assert_eq!(result.interior_walls_left_out, 1);
        assert_eq!(result.total_wall_area, 36.4);
        assert_eq!(result.overall_wwr, 0.0); // No windows in sample

        // Measured from the wall solids: the house's right walls face east,
        // its left wall west.
        let facing: Vec<(&str, usize, f64)> = result
            .orientations
            .iter()
            .map(|o| (o.direction.as_str(), o.count, o.total_area_sqm))
            .collect();
        assert_eq!(facing, vec![("East", 2, 15.2), ("West", 1, 21.2)]);
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

    #[test]
    fn a_window_listed_in_its_wall_and_in_the_model_counts_once() {
        // The IFC extractor lists a hosted window both in its wall and in
        // the model's window list.
        let model: BuildingModel = serde_json::from_value(serde_json::json!({
            "walls": [{
                "id": "wall",
                "orientation": { "x": 0.0, "y": -1.0, "z": 0.0 },
                "area": 10.0,
                "windows": [{ "id": "hosted", "area": 2.0 }]
            }],
            "windows": [
                { "id": "hosted", "area": 2.0, "hostId": "wall" },
                { "id": "loose", "area": 1.0 }
            ]
        }))
        .unwrap();
        let result = analyze_wwr(&model);
        assert_eq!(result.window_count, 2);
        assert_eq!(result.total_window_area, 3.0);
        // window / (wall + window): wall areas are net of their openings
        assert_eq!(result.overall_wwr, 23.1);
    }

    #[test]
    fn walls_flagged_interior_and_their_windows_are_left_out() {
        let model: BuildingModel = serde_json::from_value(serde_json::json!({
            "walls": [
                { "id": "out", "orientation": { "x": 0.0, "y": 1.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "windows": [{ "id": "a", "area": 2.0 }] },
                { "id": "in", "orientation": { "x": 0.0, "y": -1.0, "z": 0.0 }, "area": 5.0,
                  "isExternal": false, "windows": [{ "id": "b", "area": 1.0 }] },
                { "id": "unknown", "orientation": { "x": 1.0, "y": 0.0, "z": 0.0 }, "area": 4.0 }
            ],
            "windows": [
                { "id": "a", "area": 2.0 },
                { "id": "b", "area": 1.0 },
                { "id": "loose", "area": 0.5 }
            ]
        }))
        .unwrap();
        let result = analyze_wwr(&model);
        assert_eq!(result.wall_count, 2);
        assert_eq!(result.total_wall_area, 14.0);
        assert_eq!(result.window_count, 2);
        assert_eq!(result.total_window_area, 2.5);
        assert_eq!(result.interior_walls_left_out, 1);
        let dirs: Vec<&str> = result
            .orientations
            .iter()
            .map(|o| o.direction.as_str())
            .collect();
        assert_eq!(dirs, vec!["North", "East"]);
    }

    #[test]
    fn window_area_by_facing_follows_the_exterior_walls() {
        let model: BuildingModel = serde_json::from_value(serde_json::json!({
            "walls": [
                { "id": "east", "orientation": { "x": 1.0, "y": 0.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "windows": [{ "id": "a", "area": 2.0 }, { "id": "b", "area": 3.0 }] },
                { "id": "south", "orientation": { "x": 0.0, "y": -1.0, "z": 0.0 }, "area": 10.0,
                  "windows": [{ "id": "c", "area": 1.0 }] },
                { "id": "partition", "orientation": { "x": -1.0, "y": 0.0, "z": 0.0 }, "area": 5.0,
                  "isExternal": false, "windows": [{ "id": "d", "area": 4.0 }] },
                // a curtain wall's stored normal points inward
                { "id": "curtain", "orientation": { "x": 0.0, "y": 1.0, "z": 0.0 }, "area": 0.0,
                  "isCurtainWall": true, "windows": [{ "id": "g", "area": 6.0 }] },
                { "id": "blind", "orientation": { "x": 0.0, "y": 1.0, "z": 0.0 }, "area": 8.0 }
            ],
            "windows": [{ "id": "skylight", "area": 2.7 }]
        }))
        .unwrap();
        assert_eq!(
            window_area_by_facing(&model),
            vec![(90.0, 5.0), (180.0, 7.0)]
        );

        // on the true compass: at -90° project east is true north
        let rotated: BuildingModel = serde_json::from_value(serde_json::json!({
            "walls": [{ "orientation": { "x": 1.0, "y": 0.0, "z": 0.0 }, "area": 4.0,
                        "windows": [{ "id": "w", "area": 1.0 }] }],
            "projectNorth": -std::f64::consts::FRAC_PI_2
        }))
        .unwrap();
        assert_eq!(window_area_by_facing(&rotated), vec![(0.0, 1.0)]);
    }

    /// Direction of the only bucket for a one-wall model.
    fn facing_of(model: serde_json::Value) -> String {
        let model: BuildingModel = serde_json::from_value(model).unwrap();
        let result = analyze_wwr(&model);
        assert_eq!(result.orientations.len(), 1);
        result.orientations[0].direction.clone()
    }

    #[test]
    fn project_north_turns_walls_onto_the_true_compass() {
        // The generic format's projectNorth is the clockwise angle from
        // project to true coordinates (analysis_engine.rotate_vector_2d).
        // At -90°, true north is the project's +X: a wall facing project
        // east faces true north.
        let wall = |x: f64, y: f64| {
            serde_json::json!({
                "walls": [{ "orientation": { "x": x, "y": y, "z": 0.0 }, "area": 10.0 }],
                "projectNorth": -std::f64::consts::FRAC_PI_2
            })
        };
        assert_eq!(facing_of(wall(1.0, 0.0)), "North");
        assert_eq!(facing_of(wall(0.0, 1.0)), "West");
    }

    #[test]
    fn a_curtain_wall_normal_is_reversed() {
        // Revit reports a curtain wall's orientation pointing inward; the
        // format flags it so analyses reverse it.
        let facing = facing_of(serde_json::json!({
            "walls": [{
                "orientation": { "x": 0.0, "y": 1.0, "z": 0.0 },
                "area": 10.0,
                "isCurtainWall": true
            }]
        }));
        assert_eq!(facing, "South");
    }

    /// A fresh, empty folder for one test.
    fn folder(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("makit-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        std::fs::write(path, "ISO-10303-21;").unwrap();
    }

    /// An extraction whose metadata names `source`, or nothing at all.
    fn extraction(path: &Path, source: Option<&str>) {
        let metadata = match source {
            Some(name) => serde_json::json!({ "projectName": "p", "sourceFile": name }),
            None => serde_json::json!({ "projectName": "p" }),
        };
        let model = serde_json::json!({ "walls": [], "metadata": metadata });
        std::fs::write(path, model.to_string()).unwrap();
    }

    #[test]
    fn a_shared_extraction_is_not_given_to_another_ifc_in_its_folder() {
        let dir = folder("shared");
        touch(&dir.join("a.ifc"));
        touch(&dir.join("b.ifc"));
        extraction(&dir.join("building-model.json"), Some("a.ifc"));

        assert_eq!(
            find_json_for_ifc(&dir.join("a.ifc")),
            Some(dir.join("building-model.json"))
        );
        assert_eq!(find_json_for_ifc(&dir.join("b.ifc")), None);
    }

    #[test]
    fn the_per_file_extraction_wins_over_the_shared_one() {
        let dir = folder("per-file");
        touch(&dir.join("a.ifc"));
        touch(&dir.join("b.ifc"));
        extraction(&dir.join("building-model.json"), Some("a.ifc"));
        extraction(&dir.join("b.json"), None);

        assert_eq!(
            find_json_for_ifc(&dir.join("b.ifc")),
            Some(dir.join("b.json"))
        );
    }

    #[test]
    fn an_extraction_that_names_no_source_belongs_only_to_a_lone_ifc() {
        let dir = folder("unnamed");
        touch(&dir.join("a.ifc"));
        extraction(&dir.join("building-model.json"), None);
        assert_eq!(
            find_json_for_ifc(&dir.join("a.ifc")),
            Some(dir.join("building-model.json"))
        );

        // A second file makes it impossible to tell whose it is.
        touch(&dir.join("b.ifc"));
        assert_eq!(find_json_for_ifc(&dir.join("a.ifc")), None);
        assert_eq!(find_json_for_ifc(&dir.join("b.ifc")), None);
    }

    #[test]
    fn an_extraction_written_where_expected_is_found() {
        let dir = folder("expected");
        let ifc = dir.join("Model.IFC");
        touch(&ifc);
        touch(&dir.join("other.ifc"));
        let expected = extraction_path(&ifc);
        assert_eq!(expected, dir.join("Model.json"));

        extraction(&expected, None);
        assert_eq!(find_json_for_ifc(&ifc), Some(expected));
    }

    #[test]
    fn the_extraction_command_writes_beside_the_ifc() {
        let command = extraction_command(Path::new("examples/IFC/Ifc4_ARC.ifc"));
        assert!(command.contains("analyze_ifc.py examples/IFC/Ifc4_ARC.ifc"));
        assert!(command.ends_with("--extract-only --output examples/IFC/Ifc4_ARC.json"));
    }

    #[test]
    fn the_example_extraction_belongs_to_its_ifc_alone() {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/IFC"));
        assert_eq!(
            find_json_for_ifc(&dir.join("Building-Architecture.ifc")),
            Some(dir.join("building-model.json"))
        );
        // Any other IFC file dropped into the examples folder, such as a
        // local download, must not borrow it.
        assert_eq!(find_json_for_ifc(&dir.join("Other-Model.ifc")), None);
    }
}
