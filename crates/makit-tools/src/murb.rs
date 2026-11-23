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
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use makit_core::models::TaskContext;
use makit_core::registry::Registry;

use crate::building_model;
use crate::python;

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

/// Compass points accepted in `window-directions`, clockwise from north.
const COMPASS: [(&str, f64); 8] = [
    ("N", 0.0),
    ("NE", 45.0),
    ("E", 90.0),
    ("SE", 135.0),
    ("S", 180.0),
    ("SW", 225.0),
    ("W", 270.0),
    ("NW", 315.0),
];

const WINDOW_DIRECTIONS_FORMAT: &str = "expected azimuth:share pairs separated by commas, \
     e.g. S:60,W:40 or 143:0.67,233:0.33 (azimuth in degrees clockwise from north, \
     or N, NE, E, SE, S, SW, W, NW; shares in any unit)";

/// Parse `window-directions`: how the glazing is split between facings.
///
/// `S:60,W:40` puts 60% of the window area facing south and 40% west.
/// Azimuths are degrees clockwise from north or compass points; shares may
/// be in any unit (percentages, fractions, m² by facing) and are scaled to
/// add up to 1. An empty value keeps the default of 25% each way.
pub fn parse_window_directions(spec: &str) -> anyhow::Result<Vec<WindowGroupInput>> {
    if spec.trim().is_empty() {
        return Ok(default_window_groups());
    }
    let mut pairs = Vec::new();
    for item in spec.split(',').map(str::trim).filter(|i| !i.is_empty()) {
        let bad = || anyhow::anyhow!("bad window direction '{item}': {WINDOW_DIRECTIONS_FORMAT}");
        let (facing, share) = item.split_once(':').ok_or_else(bad)?;
        let facing = facing.trim();
        let azimuth = match COMPASS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(facing))
        {
            Some(&(_, azimuth)) => azimuth,
            None => facing
                .parse::<f64>()
                .ok()
                .filter(|a| a.is_finite())
                .ok_or_else(bad)?
                .rem_euclid(360.0),
        };
        let share = share
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|s| s.is_finite() && *s > 0.0)
            .ok_or_else(bad)?;
        pairs.push((azimuth, share));
    }
    if pairs.is_empty() {
        anyhow::bail!("no window directions: {WINDOW_DIRECTIONS_FORMAT}");
    }
    let total: f64 = pairs.iter().map(|(_, share)| share).sum();
    Ok(pairs
        .into_iter()
        .map(|(azimuth, share)| WindowGroupInput {
            pct_window_area: share / total,
            window_azimuth: azimuth,
            shgc: default_shgc(),
            shading: 0.0,
        })
        .collect())
}

/// Window groups from a building model's windows by facing. `path` is an
/// extraction JSON, or an IFC file whose extraction makit can find. Returns
/// the groups, the JSON read and the window area that faces somewhere.
pub fn window_groups_from_model(
    path: &Path,
) -> anyhow::Result<(Vec<WindowGroupInput>, PathBuf, f64)> {
    let is_ifc = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ifc"));
    let json = if is_ifc {
        building_model::find_json_for_ifc(path).ok_or_else(|| {
            anyhow::anyhow!(
                "{} is not extracted yet (no {}): extract it, or give window-directions",
                path.display(),
                building_model::extraction_path(path).display()
            )
        })?
    } else {
        path.to_path_buf()
    };
    let model = building_model::load_model(&json)
        .map_err(|e| anyhow::anyhow!("cannot read model {}: {e}", json.display()))?;
    let facings = building_model::window_area_by_facing(&model);
    let total: f64 = facings.iter().map(|(_, area)| area).sum();
    if total <= 0.0 {
        anyhow::bail!(
            "{} has no windows in exterior walls to take directions from",
            json.display()
        );
    }
    let groups = facings
        .into_iter()
        .map(|(azimuth, area)| WindowGroupInput {
            pct_window_area: area / total,
            window_azimuth: azimuth,
            shgc: default_shgc(),
            shading: 0.0,
        })
        .collect();
    Ok((groups, json, total))
}

/// The window split for `ctx` and where it came from: `window-directions`
/// when given, else the windows of `model` by facing, else 25% each way.
fn window_groups(ctx: &TaskContext) -> anyhow::Result<(Vec<WindowGroupInput>, String)> {
    let spec = ctx.get_option("window-directions", "");
    if !spec.trim().is_empty() {
        return Ok((parse_window_directions(&spec)?, "as given".to_string()));
    }
    let model = ctx.get_option("model", "");
    if model.trim().is_empty() {
        return Ok((default_window_groups(), "even split".to_string()));
    }
    let (groups, json, area) = window_groups_from_model(Path::new(model.trim()))?;
    Ok((
        groups,
        format!("from the {area:.1} m² of windows in {}", json.display()),
    ))
}

/// `68% 143°, 20% 233°, 12% 53°`
fn describe_window_groups(groups: &[WindowGroupInput]) -> String {
    groups
        .iter()
        .map(|g| format!("{:.0}% {:.0}°", g.pct_window_area * 100.0, g.window_azimuth))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Subprocess Bridge (M3-M5)
// ---------------------------------------------------------------------------

/// Run a MURB simulation via the Python subprocess bridge.
pub fn run_simulation(input: &MurbInput) -> anyhow::Result<MurbResults> {
    let python = python::find("murb_energy_tool")?;
    let script = python::repo_file("scripts/murb_runner.py")?;
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

/// The simulation input `murb-simulate` sends for `ctx`'s options, and
/// where its window split came from.
fn simulation_input(ctx: &TaskContext) -> anyhow::Result<(MurbInput, String)> {
    let (window_groups, window_source) = window_groups(ctx)?;
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
        window_groups,
    };

    if input.epw_path.is_empty() {
        anyhow::bail!("--epw is required: path to an EPW weather file");
    }
    if input.gfa <= 0.0 {
        anyhow::bail!("--gfa must be a positive number (gross floor area in m²)");
    }
    Ok((input, window_source))
}

/// Handle `murb-simulate` — runs the full subprocess bridge.
fn handle_simulate(ctx: &TaskContext) -> anyhow::Result<()> {
    let (input, window_source) = simulation_input(ctx)?;

    println!("Running MURB energy simulation...");
    println!("  EPW:      {}", input.epw_path);
    println!("  GFA:      {} m²", input.gfa);
    println!("  Province: {}", input.province);
    println!(
        "  Windows:  {} m² ({}; {})",
        input.area_windows,
        describe_window_groups(&input.window_groups),
        window_source
    );

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
fn handle_connect(_ctx: &TaskContext) -> anyhow::Result<()> {
    match python::find("murb_energy_tool") {
        Ok(python) => println!(
            "✓ MURB energy model available (Python: {})",
            python.display()
        ),
        Err(err) => {
            println!("✗ {err}");
            println!(
                "  Install: put the murb repository on PYTHONPATH, or point MAKIT_PYTHON \
                 at a Python that has murb_energy_tool"
            );
        }
    }
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
    .add_option(
        "window-directions",
        "Window share by facing, azimuth:share (e.g. S:60,W:40)",
        "string",
        false,
        None,
    )
    .add_option(
        "model",
        "IFC file or extraction to take window directions from",
        "string",
        false,
        None,
    )
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

    fn facings(groups: &[WindowGroupInput]) -> Vec<(f64, f64)> {
        groups
            .iter()
            .map(|g| {
                let share = (g.pct_window_area * 10_000.0).round() / 10_000.0;
                (g.window_azimuth, share)
            })
            .collect()
    }

    #[test]
    fn no_window_directions_means_a_quarter_each_way() {
        let quarter = vec![(0.0, 0.25), (90.0, 0.25), (180.0, 0.25), (270.0, 0.25)];
        assert_eq!(facings(&parse_window_directions("").unwrap()), quarter);
        assert_eq!(facings(&parse_window_directions("  ").unwrap()), quarter);
    }

    #[test]
    fn window_directions_take_compass_points_or_degrees_and_any_unit() {
        assert_eq!(
            facings(&parse_window_directions("S:60, w:40").unwrap()),
            vec![(180.0, 0.6), (270.0, 0.4)]
        );
        // window areas by facing, straight from a model
        assert_eq!(
            facings(&parse_window_directions("143:123.4,233:37,53:22.4").unwrap()),
            vec![(143.0, 0.6751), (233.0, 0.2024), (53.0, 0.1225)]
        );
        assert_eq!(
            facings(&parse_window_directions("NE:1,se:1,SW:1,NW:1").unwrap()),
            vec![(45.0, 0.25), (135.0, 0.25), (225.0, 0.25), (315.0, 0.25)]
        );
        assert_eq!(
            facings(&parse_window_directions("450:1,-90:1").unwrap()),
            vec![(90.0, 0.5), (270.0, 0.5)]
        );
        let group = &parse_window_directions("S:1").unwrap()[0];
        assert_eq!((group.shgc, group.shading), (0.4, 0.0));
    }

    #[test]
    fn bad_window_directions_say_what_is_expected() {
        for spec in ["S", "Q:1", "S:-1", "S:0", "S:abc", ":1", "S:1,N", "S:inf"] {
            let err = parse_window_directions(spec).unwrap_err().to_string();
            assert!(err.contains("azimuth:share"), "{spec}: {err}");
        }
    }

    #[test]
    fn murb_simulate_sends_the_window_directions() {
        let mut ctx = TaskContext::new();
        for (key, value) in [
            ("epw", "toronto.epw"),
            ("gfa", "100"),
            ("walls-ag", "50"),
            ("windows", "10"),
            ("roof", "40"),
            ("window-directions", "SE:3,SW:1"),
        ] {
            ctx.options.insert(key.into(), value.into());
        }
        let (input, _) = simulation_input(&ctx).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(135.0, 0.75), (225.0, 0.25)]
        );

        let mut reg = Registry::default();
        register_tasks(&mut reg);
        for name in ["window-directions", "model"] {
            let option = reg.actions["murb-simulate"]
                .options
                .iter()
                .find(|o| o.name == name)
                .expect("option registered");
            assert!(!option.required, "{name}");
        }
    }

    /// A fresh, empty folder for one test.
    fn folder(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("makit-murb-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A model with 3 m² of windows facing east and 1 m² facing south.
    fn write_model(path: &std::path::Path, with_windows: bool) {
        let windows = |area: f64| {
            if with_windows {
                serde_json::json!([{ "id": format!("w{area}"), "area": area }])
            } else {
                serde_json::json!([])
            }
        };
        let model = serde_json::json!({
            "walls": [
                { "id": "e", "orientation": { "x": 1.0, "y": 0.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "windows": windows(3.0) },
                { "id": "s", "orientation": { "x": 0.0, "y": -1.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "windows": windows(1.0) }
            ]
        });
        std::fs::write(path, model.to_string()).unwrap();
    }

    fn context(extra: &[(&str, &str)]) -> TaskContext {
        let mut ctx = TaskContext::new();
        for (key, value) in [
            ("epw", "toronto.epw"),
            ("gfa", "100"),
            ("walls-ag", "50"),
            ("windows", "10"),
            ("roof", "40"),
        ]
        .iter()
        .chain(extra)
        {
            ctx.options.insert(key.to_string(), value.to_string());
        }
        ctx
    }

    #[test]
    fn window_directions_come_from_the_model_when_not_given() {
        let dir = folder("from-model");
        let json = dir.join("house.json");
        write_model(&json, true);
        let (input, source) =
            simulation_input(&context(&[("model", json.to_str().unwrap())])).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
        assert!(source.contains("house.json"), "{source}");
    }

    #[test]
    fn given_window_directions_win_over_the_model() {
        let dir = folder("given-wins");
        let json = dir.join("house.json");
        write_model(&json, true);
        let (input, source) = simulation_input(&context(&[
            ("model", json.to_str().unwrap()),
            ("window-directions", "N:1"),
        ]))
        .unwrap();
        assert_eq!(facings(&input.window_groups), vec![(0.0, 1.0)]);
        assert!(!source.contains("house.json"), "{source}");
    }

    #[test]
    fn an_ifc_model_uses_its_extraction() {
        let dir = folder("ifc");
        std::fs::write(dir.join("house.ifc"), "ISO-10303-21;").unwrap();
        write_model(&dir.join("house.json"), true);
        let ifc = dir.join("house.ifc");
        let (input, _) = simulation_input(&context(&[("model", ifc.to_str().unwrap())])).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
    }

    #[test]
    fn a_model_that_cannot_give_directions_is_an_error() {
        let dir = folder("no-directions");
        let json = dir.join("blind.json");
        write_model(&json, false);
        let err = simulation_input(&context(&[("model", json.to_str().unwrap())]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no windows"), "{err}");

        std::fs::write(dir.join("other.ifc"), "ISO-10303-21;").unwrap();
        let ifc = dir.join("other.ifc");
        let err = simulation_input(&context(&[("model", ifc.to_str().unwrap())]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not extracted"), "{err}");
    }

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
