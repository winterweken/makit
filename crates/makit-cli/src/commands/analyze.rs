//! `makit analyze` — analyze a geometry or IFC file.

use anyhow::Result;

pub fn run(file: &str, analysis_type: &str) -> Result<()> {
    println!("Analyzing: {file}");
    println!("Analysis type: {analysis_type}");
    println!("TODO: implement IFC/geometry analysis");
    Ok(())
}
