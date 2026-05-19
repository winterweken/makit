//! `makit analyze` — analyze a geometry or IFC file.

use std::path::Path;

use anyhow::Result;
use makit_tools::building_model;

pub fn run(file: &str, analysis_type: &str) -> Result<()> {
    let path = Path::new(file);

    if !path.exists() {
        anyhow::bail!("File not found: {}", file);
    }

    // If given an IFC file, look for the companion JSON
    let json_path = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ifc"))
    {
        match building_model::find_json_for_ifc(path) {
            Some(p) => {
                println!("  Found extracted model: {}", p.display());
                p
            }
            None => {
                anyhow::bail!(
                    "No extracted JSON found for {}.\n\
                     Run the Python extractor first:\n\
                     python3 pyrevit-extension/Makit.extension/lib/analyze_ifc.py {} --extract-only --output building-model.json",
                    file, file
                );
            }
        }
    } else {
        path.to_path_buf()
    };

    let model = building_model::load_model(&json_path)?;

    match analysis_type {
        "wwr" | "wall-orientation" | "orientation" => {
            let analysis = building_model::analyze_wwr(&model);
            building_model::print_wwr_report(&analysis);
        }
        _ => {
            // Default: print summary + WWR
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

            let analysis = building_model::analyze_wwr(&model);
            building_model::print_wwr_report(&analysis);
        }
    }

    Ok(())
}
