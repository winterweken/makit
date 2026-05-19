//! MURB energy modelling tool bridge.
//!
//! Bridges the Python murb_energy_tool via JSON subprocess for
//! early-stage TEDI/TEUI/GHGI analysis.
//!
//! Protocol:
//! 1. Serialize `MurbInput` as JSON
//! 2. Spawn `python scripts/murb_runner.py`
//! 3. Write JSON to child stdin, close it
//! 4. Read JSON from child stdout
//! 5. Deserialize into `MurbResults`

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use makit_core::models::TaskContext;
use makit_core::registry::Registry;

// ---------------------------------------------------------------------------
// Data Types (M2)
// ---------------------------------------------------------------------------

/// Input parameters for a MURB energy simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MurbInput {
    pub epw_path: String,
    pub name: String,
    pub province: String,
    pub gfa: f64,
    pub area_walls_ag: f64,
    #[serde(default)]
    pub area_walls_bg: f64,
    pub area_windows: f64,
    pub area_roof: f64,
    #[serde(default = "default_u_walls")]
    pub u_walls_ag: f64,
    #[serde(default = "default_u_windows")]
    pub u_windows: f64,
    #[serde(default = "default_u_roof")]
    pub u_roof: f64,
    #[serde(default = "default_cop_htg")]
    pub cop_htg: f64,
    #[serde(default = "default_cop_clg")]
    pub cop_clg: f64,
    #[serde(default = "default_cop_dhw")]
    pub cop_dhw: f64,
    #[serde(default = "default_hrv")]
    pub hrv_efficiency: f64,
    #[serde(default = "default_window_groups")]
    pub window_groups: Vec<WindowGroupInput>,
}

/// A window group for solar gain calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowGroupInput {
    pub pct_window_area: f64,
    pub window_azimuth: f64,
    #[serde(default = "default_shgc")]
    pub shgc: f64,
    #[serde(default)]
    pub shading: f64,
}

/// Results from a MURB energy simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MurbResults {
    pub name: String,
    pub monthly_heating_demand: Vec<f64>,
    pub monthly_cooling_demand: Vec<f64>,
    pub monthly_heating_consumption: Vec<f64>,
    pub monthly_cooling_consumption: Vec<f64>,
    pub monthly_total_consumption: Vec<f64>,
    pub monthly_ghg_emissions: Vec<f64>,
    pub tedi_kwh_m2: f64,
    pub teui_kwh_m2: f64,
    pub ghgi_kg_m2: f64,
    pub gfa: f64,
    pub weather_file: String,
}

// Default value functions for serde
fn default_u_walls() -> f64 {
    0.273
}
fn default_u_windows() -> f64 {
    2.56
}
fn default_u_roof() -> f64 {
    0.164
}
fn default_cop_htg() -> f64 {
    0.85
}
fn default_cop_clg() -> f64 {
    5.2
}
fn default_cop_dhw() -> f64 {
    0.85
}
fn default_hrv() -> f64 {
    0.55
}
fn default_shgc() -> f64 {
    0.4
}

fn default_window_groups() -> Vec<WindowGroupInput> {
    vec![
        WindowGroupInput {
            pct_window_area: 0.25,
            window_azimuth: 0.0,
            shgc: 0.4,
            shading: 0.0,
        },
        WindowGroupInput {
            pct_window_area: 0.25,
            window_azimuth: 90.0,
            shgc: 0.4,
            shading: 0.0,
        },
        WindowGroupInput {
            pct_window_area: 0.25,
            window_azimuth: 180.0,
            shgc: 0.4,
            shading: 0.0,
        },
        WindowGroupInput {
            pct_window_area: 0.25,
            window_azimuth: 270.0,
            shgc: 0.4,
            shading: 0.0,
        },
    ]
}

// ---------------------------------------------------------------------------
// Subprocess Bridge (M3-M5)
// ---------------------------------------------------------------------------

/// Locate the murb_runner.py script relative to the makit binary.
fn find_runner_script() -> anyhow::Result<PathBuf> {
    // Check relative to current executable
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));
        // Development: binary is in target/debug, script is in scripts/
        for candidate in &[
            exe_dir.join("../../scripts/murb_runner.py"),
            exe_dir.join("../scripts/murb_runner.py"),
            exe_dir.join("scripts/murb_runner.py"),
        ] {
            if candidate.exists() {
                return Ok(candidate.canonicalize()?);
            }
        }
    }

    // Check relative to CWD
    let cwd_script = PathBuf::from("scripts/murb_runner.py");
    if cwd_script.exists() {
        return Ok(cwd_script.canonicalize()?);
    }

    anyhow::bail!(
        "murb_runner.py not found. Ensure the makit repository is intact \
         and scripts/murb_runner.py exists."
    )
}

/// Find a Python interpreter on the system.
fn find_python() -> anyhow::Result<String> {
    for candidate in &["python3", "python"] {
        if Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!("Python not found — MURB requires Python 3.10+ with murb_energy_tool installed")
}

/// Run a MURB simulation via the Python subprocess bridge.
pub fn run_simulation(input: &MurbInput) -> anyhow::Result<MurbResults> {
    let python = find_python()?;
    let script = find_runner_script()?;
    let input_json = serde_json::to_string(input)?;

    let mut child = Command::new(&python)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn Python process: {}", e))?;

    // Write JSON to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes())?;
        // stdin is dropped here, closing the pipe
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("MURB simulation failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 from murb_runner: {}", e))?;

    let results: MurbResults = serde_json::from_str(&stdout).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse MURB results: {} — output: {}",
            e,
            &stdout[..stdout.len().min(200)]
        )
    })?;

    Ok(results)
}

/// Format MURB results as a text report.
pub fn format_report(results: &MurbResults) -> String {
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut report = String::new();

    report.push_str("╔══════════════════════════════════════════╗\n");
    report.push_str(&format!("║  MURB Energy Report: {:<19} ║\n", results.name));
    report.push_str("╠══════════════════════════════════════════╣\n");
    report.push_str(&format!("║  Weather: {:<30} ║\n", results.weather_file));
    report.push_str(&format!(
        "║  GFA:     {:<10.0} m²                 ║\n",
        results.gfa
    ));
    report.push_str("╠══════════════════════════════════════════╣\n");
    report.push_str(&format!(
        "║  TEDI:    {:<10.2} kWh/m²             ║\n",
        results.tedi_kwh_m2
    ));
    report.push_str(&format!(
        "║  TEUI:    {:<10.2} kWh/m²             ║\n",
        results.teui_kwh_m2
    ));
    report.push_str(&format!(
        "║  GHGI:    {:<10.2} kgCO₂/m²           ║\n",
        results.ghgi_kg_m2
    ));
    report.push_str("╠══════════════════════════════════════════╣\n");
    report.push_str("║  Month   Heating   Cooling   Total kWh  ║\n");
    report.push_str("║  ─────   ───────   ───────   ─────────  ║\n");

    for (i, m) in months.iter().enumerate() {
        let htg = results.monthly_heating_demand.get(i).unwrap_or(&0.0);
        let clg = results.monthly_cooling_demand.get(i).unwrap_or(&0.0);
        let total = results.monthly_total_consumption.get(i).unwrap_or(&0.0);
        report.push_str(&format!(
            "║  {:<5} {:>9.1} {:>9.1} {:>11.1}  ║\n",
            m, htg, clg, total
        ));
    }

    report.push_str("╚══════════════════════════════════════════╝\n");
    report
}

// ---------------------------------------------------------------------------
// Handler Bodies (M5, M6)
// ---------------------------------------------------------------------------

/// Handle `murb-simulate` — runs the full subprocess bridge.
fn handle_simulate(ctx: &TaskContext) -> anyhow::Result<()> {
    let input = MurbInput {
        epw_path: ctx.get_option("epw", ""),
        name: ctx.get_option("name", "makit_run"),
        province: ctx.get_option("province", "ON"),
        gfa: ctx.get_option("gfa", "0").parse()?,
        area_walls_ag: ctx.get_option("walls-ag", "0").parse()?,
        area_walls_bg: ctx.get_option("walls-bg", "0").parse()?,
        area_windows: ctx.get_option("windows", "0").parse()?,
        area_roof: ctx.get_option("roof", "0").parse()?,
        u_walls_ag: ctx.get_option("u-walls", "0.273").parse()?,
        u_windows: ctx.get_option("u-windows", "2.56").parse()?,
        u_roof: ctx.get_option("u-roof", "0.164").parse()?,
        cop_htg: ctx.get_option("cop-htg", "0.85").parse()?,
        cop_clg: ctx.get_option("cop-clg", "5.2").parse()?,
        cop_dhw: ctx.get_option("cop-dhw", "0.85").parse()?,
        hrv_efficiency: ctx.get_option("hrv", "0.55").parse()?,
        window_groups: default_window_groups(),
    };

    if input.epw_path.is_empty() {
        anyhow::bail!("--epw is required: path to an EPW weather file");
    }
    if input.gfa <= 0.0 {
        anyhow::bail!("--gfa must be a positive number (gross floor area in m²)");
    }

    println!("Running MURB energy simulation...");
    println!("  EPW:      {}", input.epw_path);
    println!("  GFA:      {} m²", input.gfa);
    println!("  Province: {}", input.province);

    let results = run_simulation(&input)?;

    // Print summary
    println!();
    print!("{}", format_report(&results));

    // Save to file if requested
    let output_path = ctx.get_option("output", "murb_results.json");
    if !output_path.is_empty() {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)?;
        println!("\nResults saved to: {}", output_path);
    }

    Ok(())
}

/// Handle `murb-report` — load results JSON and print formatted report.
fn handle_report(ctx: &TaskContext) -> anyhow::Result<()> {
    let input_path = ctx.get_option("input", "");
    if input_path.is_empty() {
        anyhow::bail!("--input is required: path to MURB results JSON");
    }

    let json_str = std::fs::read_to_string(&input_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input_path, e))?;

    let results: MurbResults = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Invalid MURB results JSON: {}", e))?;

    let format = ctx.get_option("format", "text");
    match format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        _ => {
            print!("{}", format_report(&results));
        }
    }

    Ok(())
}

/// Handle `murb` source connection check.
fn handle_connect(ctx: &TaskContext) -> anyhow::Result<()> {
    // Verify Python + murb_energy_tool are available
    let python = find_python()?;
    let check = Command::new(&python)
        .args(["-c", "import murb_energy_tool; print('ok')"])
        .output()?;

    if check.status.success() {
        println!("✓ MURB energy model available (Python: {})", python);
    } else {
        let stderr = String::from_utf8_lossy(&check.stderr);
        println!("✗ murb_energy_tool not found: {}", stderr.trim());
        println!("  Install: pip install -r requirements.txt (from murb repo)");
    }

    let _ = ctx; // suppress unused warning
    Ok(())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source(
        "murb",
        "MURB energy modelling tool",
        Arc::new(handle_connect),
    );

    reg.register_action(
        "murb-simulate",
        "Run energy simulation",
        "analysis",
        Arc::new(handle_simulate),
    )
    .add_option("epw", "Path to EPW weather file", "string", true, None)
    .add_option(
        "name",
        "Simulation name",
        "string",
        false,
        Some("makit_run"),
    )
    .add_option("gfa", "Gross floor area [m²]", "float", true, None)
    .add_option(
        "walls-ag",
        "Above-grade wall area [m²]",
        "float",
        true,
        None,
    )
    .add_option(
        "walls-bg",
        "Below-grade wall area [m²]",
        "float",
        false,
        Some("0"),
    )
    .add_option("windows", "Window area [m²]", "float", true, None)
    .add_option("roof", "Roof area [m²]", "float", true, None)
    .add_option(
        "u-walls",
        "Wall U-value [W/m²K]",
        "float",
        false,
        Some("0.273"),
    )
    .add_option(
        "u-windows",
        "Window U-value [W/m²K]",
        "float",
        false,
        Some("2.56"),
    )
    .add_option(
        "u-roof",
        "Roof U-value [W/m²K]",
        "float",
        false,
        Some("0.164"),
    )
    .add_option("cop-htg", "Heating COP", "float", false, Some("0.85"))
    .add_option("cop-clg", "Cooling COP", "float", false, Some("5.2"))
    .add_option("cop-dhw", "DHW COP", "float", false, Some("0.85"))
    .add_option("hrv", "HRV efficiency (0-1)", "float", false, Some("0.55"))
    .add_option(
        "province",
        "Canadian province code",
        "string",
        false,
        Some("ON"),
    )
    .add_option(
        "output",
        "Output JSON path",
        "string",
        false,
        Some("murb_results.json"),
    );

    reg.register_action(
        "murb-report",
        "Generate energy report",
        "reporting",
        Arc::new(handle_report),
    )
    .add_option("input", "Simulation results JSON", "string", true, None)
    .add_option(
        "format",
        "Report format (text, json)",
        "string",
        false,
        Some("text"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_murb_input_serialization() {
        let input = MurbInput {
            epw_path: "/tmp/test.epw".to_string(),
            name: "test".to_string(),
            province: "ON".to_string(),
            gfa: 5000.0,
            area_walls_ag: 2400.0,
            area_walls_bg: 0.0,
            area_windows: 800.0,
            area_roof: 1000.0,
            u_walls_ag: 0.273,
            u_windows: 2.56,
            u_roof: 0.164,
            cop_htg: 0.85,
            cop_clg: 5.2,
            cop_dhw: 0.85,
            hrv_efficiency: 0.55,
            window_groups: default_window_groups(),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"gfa\":5000.0"));
        assert!(json.contains("\"province\":\"ON\""));

        // Round-trip
        let parsed: MurbInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.gfa, 5000.0);
        assert_eq!(parsed.window_groups.len(), 4);
    }

    #[test]
    fn test_murb_results_deserialization() {
        let json = r#"{
            "name": "test_TMY",
            "monthly_heating_demand": [100,90,60,20,0,0,0,0,0,30,70,95],
            "monthly_cooling_demand": [0,0,0,0,5,15,20,18,8,0,0,0],
            "monthly_heating_consumption": [118,106,71,24,0,0,0,0,0,35,82,112],
            "monthly_cooling_consumption": [0,0,0,0,1,3,4,3,2,0,0,0],
            "monthly_total_consumption": [150,140,110,70,50,60,65,62,55,80,120,145],
            "monthly_ghg_emissions": [10,9,7,4,3,3,4,3,3,5,8,9],
            "tedi_kwh_m2": 93.0,
            "teui_kwh_m2": 221.4,
            "ghgi_kg_m2": 13.6,
            "gfa": 5000.0,
            "weather_file": "TORONTO"
        }"#;

        let results: MurbResults = serde_json::from_str(json).unwrap();
        assert_eq!(results.name, "test_TMY");
        assert_eq!(results.tedi_kwh_m2, 93.0);
        assert_eq!(results.monthly_heating_demand.len(), 12);
    }

    #[test]
    fn test_format_report() {
        let results = MurbResults {
            name: "Test_TMY".to_string(),
            monthly_heating_demand: vec![100.0; 12],
            monthly_cooling_demand: vec![10.0; 12],
            monthly_heating_consumption: vec![118.0; 12],
            monthly_cooling_consumption: vec![2.0; 12],
            monthly_total_consumption: vec![150.0; 12],
            monthly_ghg_emissions: vec![8.0; 12],
            tedi_kwh_m2: 93.0,
            teui_kwh_m2: 221.4,
            ghgi_kg_m2: 13.6,
            gfa: 5000.0,
            weather_file: "TORONTO".to_string(),
        };

        let report = format_report(&results);
        assert!(report.contains("TEDI"));
        assert!(report.contains("93.00"));
        assert!(report.contains("TORONTO"));
    }
}
