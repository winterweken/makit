//! `makit analyze` — analyze a geometry or IFC file.

use std::path::Path;

use anyhow::Result;
use makit_tools::{building_model, ifc};

/// Analyse `file`: an IFC file (extracted on demand) or an extraction JSON.
/// `analysis_type` is `summary` (metadata, WWR and envelope), `wwr` (or
/// `wall-orientation`, `orientation`) or `envelope`; `grade` is the ground
/// line for the wall split, metres in project coordinates.
pub fn run(file: &str, analysis_type: &str, grade: f64) -> Result<()> {
    let path = Path::new(file);

    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }

    // An IFC file is extracted beside itself when needed; its JSON is analysed.
    let json_path = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ifc"))
    {
        let resolved = ifc::extraction_for(path, None)?;
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

    let model = building_model::load_model(&json_path)?;

    match analysis_type {
        "wwr" | "wall-orientation" | "orientation" => {
            let analysis = building_model::analyze_wwr(&model);
            building_model::print_wwr_report(&analysis);
        }
        "envelope" => {
            building_model::print_envelope(&building_model::envelope(&model, grade));
        }
        _ => {
            // Default: summary, WWR and envelope
            println!("\n  File: {}", json_path.display());
            if let Some(ref meta) = model.metadata {
                if !meta.project_name.is_empty() {
                    println!("  Project: {}", meta.project_name);
                }
                if !meta.description.is_empty() {
                    println!("  Description: {}", meta.description);
                }
            }
            println!("  Units: {}", model.units);
            println!("  Walls: {}", model.walls.len());
            println!("  Windows: {}", model.windows.len());
            println!("  Storeys: {}", model.storeys.len());
            println!("  Slabs: {}", model.slabs.len());

            let analysis = building_model::analyze_wwr(&model);
            building_model::print_wwr_report(&analysis);
            building_model::print_envelope(&building_model::envelope(&model, grade));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/IFC/building-model.json"
    );

    #[test]
    fn every_analysis_type_runs_on_a_json_model() {
        for kind in ["summary", "wwr", "envelope"] {
            run(SAMPLE, kind, 0.0).unwrap_or_else(|e| panic!("{kind}: {e}"));
        }
        run(SAMPLE, "envelope", -1.5).unwrap();
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let err = run("/nowhere/missing.ifc", "summary", 0.0)
            .unwrap_err()
            .to_string();
        assert!(err.contains("File not found"), "{err}");
    }
}
