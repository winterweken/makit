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

use makit_core::models::{CancelToken, TaskContext};
use makit_core::registry::Registry;

use crate::building_model::{self, Envelope};
use crate::ifc;
use crate::murb_report;
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
    /// Directory under which the runner writes murb's own HTML report
    /// (`results/<name>/<name>.html`); none, no report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_dir: Option<String>,
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
    /// murb's HTML report, when the runner wrote one.
    #[serde(default)]
    pub report_path: Option<String>,
    /// Why it could not write one, when it could not.
    #[serde(default)]
    pub report_error: Option<String>,
    /// The input the run was made from, for the report page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<MurbInput>,
    /// The model's envelope, when a model gave the areas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope: Option<Envelope>,
    /// The `--model` path, when one was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// A second run with the glass spread evenly, for comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_glass: Option<EvenGlass>,
    /// The makit command that reproduces the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// What a run with the glass spread evenly (25 % each way) gave, kept with
/// the results so the report page can show what the real split is worth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvenGlass {
    pub tedi_kwh_m2: f64,
    pub teui_kwh_m2: f64,
    pub ghgi_kg_m2: f64,
    pub monthly_heating_demand: Vec<f64>,
    pub monthly_cooling_demand: Vec<f64>,
    pub monthly_total_consumption: Vec<f64>,
}

impl EvenGlass {
    fn from_results(results: &MurbResults) -> Self {
        Self {
            tedi_kwh_m2: results.tedi_kwh_m2,
            teui_kwh_m2: results.teui_kwh_m2,
            ghgi_kg_m2: results.ghgi_kg_m2,
            monthly_heating_demand: results.monthly_heating_demand.clone(),
            monthly_cooling_demand: results.monthly_cooling_demand.clone(),
            monthly_total_consumption: results.monthly_total_consumption.clone(),
        }
    }
}

/// Whether `groups` is the even split murb defaults to: a quarter each way.
fn is_even_split(groups: &[WindowGroupInput]) -> bool {
    let even = default_window_groups();
    groups.len() == even.len()
        && groups.iter().zip(&even).all(|(g, e)| {
            (g.pct_window_area - e.pct_window_area).abs() < 1e-9
                && (g.window_azimuth - e.window_azimuth).abs() < 1e-9
        })
}

/// The comparison run for `input`: the same building with its glass spread
/// evenly, named after it and without murb's HTML report. `None` when the
/// split already is even.
pub fn even_glass_input(input: &MurbInput) -> Option<MurbInput> {
    if is_even_split(&input.window_groups) {
        return None;
    }
    Some(MurbInput {
        name: format!("{} (even glass)", input.name),
        window_groups: default_window_groups(),
        report_dir: None,
        ..input.clone()
    })
}

/// The makit command that reproduces a run: every option given, sorted,
/// values quoted when the shell would need it.
pub fn command_line(ctx: &TaskContext) -> String {
    let mut keys: Vec<&String> = ctx.options.keys().collect();
    keys.sort();
    let mut parts = vec!["makit exec murb analysis murb-simulate".to_string()];
    for key in keys {
        let value = &ctx.options[key];
        if value.is_empty() {
            continue;
        }
        let needs_quotes = value
            .chars()
            .any(|c| c.is_whitespace() || "'\"$`\\".contains(c));
        let value = if needs_quotes {
            format!("'{}'", value.replace('\'', "'\\''"))
        } else {
            value.clone()
        };
        parts.push(format!("-o {key}={value}"));
    }
    parts.join(" ")
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

/// Window groups for window areas by facing: shares of the total, or the
/// even split when nothing faces anywhere.
fn groups_from_facings(facings: &[(f64, f64)]) -> Vec<WindowGroupInput> {
    let total: f64 = facings.iter().map(|(_, area)| area).sum();
    if total <= 0.0 {
        return default_window_groups();
    }
    facings
        .iter()
        .map(|&(azimuth, area)| WindowGroupInput {
            pct_window_area: area / total,
            window_azimuth: azimuth,
            shgc: default_shgc(),
            shading: 0.0,
        })
        .collect()
}

/// The JSON for a `--model` path: the file itself, or an IFC file's
/// extraction, made or refreshed when needed (a note says when a stale one
/// had to do).
fn model_json(path: &Path, cancel: Option<&CancelToken>) -> anyhow::Result<PathBuf> {
    let is_ifc = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ifc"));
    if !is_ifc {
        return Ok(path.to_path_buf());
    }
    let resolved = ifc::extraction_for(path, cancel)?;
    if let Some(note) = resolved.note {
        println!("  {note}");
    }
    Ok(resolved.json)
}

/// Window groups from a building model's windows by facing. `path` is an
/// extraction JSON, or an IFC file (extracted on demand). Returns the
/// groups, the JSON read and the window area that faces somewhere; without
/// any, the even split and 0.
pub fn window_groups_from_model(
    path: &Path,
) -> anyhow::Result<(Vec<WindowGroupInput>, PathBuf, f64)> {
    let json = model_json(path, None)?;
    let model = building_model::load_model(&json)
        .map_err(|e| anyhow::anyhow!("cannot read model {}: {e}", json.display()))?;
    let facings = building_model::window_area_by_facing(&model);
    let total: f64 = facings.iter().map(|(_, area)| area).sum();
    Ok((groups_from_facings(&facings), json, total))
}

/// The model named by `--model`, loaded once: its envelope at `grade` and
/// the JSON it came from. `None` without a model.
fn model_envelope(ctx: &TaskContext, grade: f64) -> anyhow::Result<Option<(Envelope, PathBuf)>> {
    let option = ctx.get_option("model", "");
    let path = option.trim();
    if path.is_empty() {
        return Ok(None);
    }
    let json = model_json(Path::new(path), Some(&ctx.cancel))?;
    let model = building_model::load_model(&json)
        .map_err(|e| anyhow::anyhow!("cannot read model {}: {e}", json.display()))?;
    Ok(Some((building_model::envelope(&model, grade), json)))
}

/// One envelope area `murb-simulate` needs and how it resolves.
struct Quantity {
    option: &'static str,
    /// Column label in the summary.
    label: &'static str,
    /// What the model lacks when it gives zero, for the error.
    what: &'static str,
    pick: fn(&Envelope) -> f64,
    /// A zero from the model is a gap (every building has this), not a value.
    zero_is_gap: bool,
    /// What to use without a model or a typed value; `None` is an error.
    default: Option<f64>,
}

/// The five areas, in the order they are resolved and reported.
fn quantities() -> [Quantity; 5] {
    [
        Quantity {
            option: "gfa",
            label: "GFA",
            what: "storeys",
            pick: |e| e.gfa,
            zero_is_gap: true,
            default: None,
        },
        Quantity {
            option: "walls-ag",
            label: "Walls AG",
            what: "above-grade walls",
            pick: |e| e.walls_ag,
            zero_is_gap: true,
            default: None,
        },
        Quantity {
            option: "walls-bg",
            label: "Walls BG",
            what: "below-grade walls",
            pick: |e| e.walls_bg,
            zero_is_gap: false,
            default: Some(0.0),
        },
        Quantity {
            option: "windows",
            label: "Windows",
            what: "windows",
            pick: |e| e.windows,
            zero_is_gap: false,
            default: None,
        },
        Quantity {
            option: "roof",
            label: "Roof",
            what: "roof",
            pick: |e| e.roof,
            zero_is_gap: true,
            default: None,
        },
    ]
}

/// What one quantity is and where it came from: typed wins, else the model,
/// else the default; a gap names the option.
fn resolve_area(
    ctx: &TaskContext,
    quantity: &Quantity,
    model: Option<&(Envelope, PathBuf)>,
) -> anyhow::Result<(f64, String)> {
    let option = quantity.option;
    let typed = ctx.get_option(option, "");
    let typed = typed.trim();
    if !typed.is_empty() {
        let value: f64 = typed
            .parse()
            .map_err(|_| anyhow::anyhow!("--{option} must be a number, not '{typed}'"))?;
        return Ok((value, "typed".to_string()));
    }
    match model {
        Some((envelope, json)) => {
            let value = (quantity.pick)(envelope);
            if value <= 0.0 && quantity.zero_is_gap {
                if envelope.format_version < 2 {
                    anyhow::bail!(
                        "{option}: {} predates storeys and slabs; re-extract it with makit extract",
                        json.display()
                    );
                }
                anyhow::bail!(
                    "{option}: the model {} has no {}; give --{option}",
                    json.display(),
                    quantity.what
                );
            }
            Ok((value, format!("from {}", json.display())))
        }
        None => match quantity.default {
            Some(value) => Ok((value, "default".to_string())),
            None => anyhow::bail!("{option}: give --{option} or --model"),
        },
    }
}

/// The window split for `ctx` and where it came from: `window-directions`
/// when given, else the windows of the model by facing, else 25% each way.
fn window_groups(
    ctx: &TaskContext,
    model: Option<&(Envelope, PathBuf)>,
) -> anyhow::Result<(Vec<WindowGroupInput>, String)> {
    let spec = ctx.get_option("window-directions", "");
    if !spec.trim().is_empty() {
        return Ok((parse_window_directions(&spec)?, "as given".to_string()));
    }
    let Some((envelope, json)) = model else {
        return Ok((default_window_groups(), "even split".to_string()));
    };
    let total: f64 = envelope
        .windows_by_facing
        .iter()
        .map(|(_, area)| area)
        .sum();
    if total <= 0.0 {
        return Ok((
            default_window_groups(),
            format!("even split: no windows with a facing in {}", json.display()),
        ));
    }
    Ok((
        groups_from_facings(&envelope.windows_by_facing),
        format!("from the {total:.1} m² of windows in {}", json.display()),
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

/// The weather station an EPW file is for, from its `LOCATION` header:
/// the city up to its first dot ("Toronto" for "Toronto.City-Univ.of.Toronto").
pub fn epw_city(path: &Path) -> Option<String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let first = std::io::BufReader::new(file).lines().next()?.ok()?;
    let fields = first.strip_prefix("LOCATION,")?;
    let city = fields.split(',').next()?.split('.').next()?.trim();
    (!city.is_empty()).then(|| city.to_string())
}

/// The run's name: as typed, else the model's file name (or `makit_run`)
/// "in" the EPW's city when the EPW can be read: "Ifc4_ARC in Toronto".
fn run_name(typed: &str, model: Option<&Path>, epw: &Path) -> String {
    if !typed.trim().is_empty() {
        return typed.trim().to_string();
    }
    let base = model
        .and_then(|m| m.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "makit_run".to_string());
    match epw_city(epw) {
        Some(city) => format!("{base} in {city}"),
        None => base,
    }
}

/// Where murb's HTML report goes: the directory of the results file
/// (`--output`), made absolute; none when no results file is written.
fn report_dir(output: &str) -> Option<String> {
    let output = output.trim();
    if output.is_empty() {
        return None;
    }
    let parent = Path::new(output)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = if parent.is_absolute() {
        parent
    } else {
        std::env::current_dir().ok()?.join(&parent)
    };
    // "." and ".." on the end of an absolute path, without touching the disk
    let mut clean = PathBuf::new();
    for part in dir.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    Some(clean.to_string_lossy().into_owned())
}

/// The simulation input `murb-simulate` sends for `ctx`'s options, one line
/// per quantity saying where it came from, and the model's envelope when a
/// model was given.
fn simulation_input(
    ctx: &TaskContext,
) -> anyhow::Result<(MurbInput, Vec<String>, Option<Envelope>)> {
    let grade_text = ctx.get_option("grade", "0");
    let grade: f64 = grade_text.trim().parse().map_err(|_| {
        anyhow::anyhow!("--grade must be a number (metres, project z), not '{grade_text}'")
    })?;
    let model = model_envelope(ctx, grade)?;

    let mut sources = Vec::new();
    let mut areas = [0.0; 5];
    for (slot, quantity) in areas.iter_mut().zip(quantities().iter()) {
        let (value, source) = resolve_area(ctx, quantity, model.as_ref())?;
        sources.push(format!("{:<9}{value:>9.1} m²  {source}", quantity.label));
        *slot = value;
    }
    let [gfa, walls_ag, walls_bg, windows, roof] = areas;
    let (window_groups, window_source) = window_groups(ctx, model.as_ref())?;
    sources.push(format!(
        "{:<9}{}  {window_source}",
        "Split",
        describe_window_groups(&window_groups)
    ));

    let epw_path = ctx.get_option("epw", "");
    let name = run_name(
        &ctx.get_option("name", ""),
        model.as_ref().map(|(_, json)| json.as_path()),
        Path::new(&epw_path),
    );
    let input = MurbInput {
        epw_path,
        name,
        province: ctx.get_option("province", "ON"),
        gfa,
        area_walls_ag: walls_ag,
        area_walls_bg: walls_bg,
        area_windows: windows,
        area_roof: roof,
        u_walls_ag: ctx.get_option("u-walls", "0.273").parse()?,
        u_windows: ctx.get_option("u-windows", "2.56").parse()?,
        u_roof: ctx.get_option("u-roof", "0.164").parse()?,
        cop_htg: ctx.get_option("cop-htg", "0.85").parse()?,
        cop_clg: ctx.get_option("cop-clg", "5.2").parse()?,
        cop_dhw: ctx.get_option("cop-dhw", "0.85").parse()?,
        hrv_efficiency: ctx.get_option("hrv", "0.55").parse()?,
        window_groups,
        report_dir: report_dir(&ctx.get_option("output", "murb_results.json")),
    };

    if input.epw_path.is_empty() {
        anyhow::bail!("--epw is required: path to an EPW weather file");
    }
    if input.gfa <= 0.0 {
        anyhow::bail!("--gfa must be a positive number (gross floor area in m²)");
    }
    Ok((input, sources, model.map(|(envelope, _)| envelope)))
}

/// Handle `murb-simulate` — runs the full subprocess bridge.
fn handle_simulate(ctx: &TaskContext) -> anyhow::Result<()> {
    let (input, sources, envelope) = simulation_input(ctx)?;

    println!("Running MURB energy simulation...");
    println!("  EPW:      {}", input.epw_path);
    println!("  Province: {}", input.province);
    if let Some(envelope) = &envelope {
        building_model::print_envelope(envelope);
    }
    println!();
    for line in &sources {
        println!("  {line}");
    }
    println!();

    let mut results = run_simulation(&input)?;

    // Print summary
    print!("{}", format_report(&results));
    if let Some(path) = &results.report_path {
        println!("\nmurb's HTML report: {path}");
    } else if let Some(err) = &results.report_error {
        println!("\n{err}");
    }

    // The same building with its glass spread evenly, for the page's comparison
    if let Some(even) = even_glass_input(&input) {
        match run_simulation(&even) {
            Ok(comparison) => results.even_glass = Some(EvenGlass::from_results(&comparison)),
            Err(err) => println!("\n(no even-glass comparison: {err:#})"),
        }
    }
    let model = ctx.get_option("model", "");
    results.model = (!model.trim().is_empty()).then(|| model.trim().to_string());
    results.envelope = envelope;
    results.command = Some(command_line(ctx));
    results.input = Some(input);

    // Save the results and the report page beside them
    let output_path = ctx.get_option("output", "murb_results.json");
    if !output_path.is_empty() {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)?;
        println!("\nResults saved to: {}", output_path);
        let page = murb_report::page_path_for(Path::new(&output_path));
        std::fs::write(&page, murb_report::render_html(&results))?;
        println!("Report page:      {}", page.display());
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
        "html" => {
            let output = ctx.get_option("output", "");
            let page = if output.trim().is_empty() {
                murb_report::page_path_for(Path::new(&input_path))
            } else {
                PathBuf::from(output.trim())
            };
            std::fs::write(&page, murb_report::render_html(&results))?;
            println!("Report page: {}", page.display());
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
        "Simulation name (default: the model's name in the EPW's city)",
        "string",
        false,
        None,
    )
    .add_option(
        "gfa",
        "Gross floor area [m²] (from the model when not given)",
        "float",
        false,
        None,
    )
    .add_option(
        "walls-ag",
        "Above-grade wall area [m²] (from the model when not given)",
        "float",
        false,
        None,
    )
    .add_option(
        "walls-bg",
        "Below-grade wall area [m²] (from the model when not given, else 0)",
        "float",
        false,
        None,
    )
    .add_option(
        "windows",
        "Window area [m²] (from the model when not given)",
        "float",
        false,
        None,
    )
    .add_option(
        "window-directions",
        "Window share by facing, azimuth:share (e.g. S:60,W:40)",
        "string",
        false,
        None,
    )
    .add_option(
        "model",
        "IFC file or extraction: envelope areas and window directions come from it",
        "string",
        false,
        None,
    )
    .add_option(
        "grade",
        "Grade elevation [m] in project coordinates; wall area below it counts as below-grade",
        "float",
        false,
        Some("0"),
    )
    .add_option(
        "roof",
        "Roof area [m²] (from the model when not given)",
        "float",
        false,
        None,
    )
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
        "Report format (text, json, html)",
        "string",
        false,
        Some("text"),
    )
    .add_option(
        "output",
        "Where html writes the page (default: beside the input)",
        "string",
        false,
        None,
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
        let (input, _, _) = simulation_input(&ctx).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(135.0, 0.75), (225.0, 0.25)]
        );

        let mut reg = Registry::default();
        register_tasks(&mut reg);
        let option = |name: &str| {
            reg.actions["murb-simulate"]
                .options
                .iter()
                .find(|o| o.name == name)
                .unwrap_or_else(|| panic!("{name} registered"))
                .clone()
        };
        for name in [
            "window-directions",
            "model",
            "gfa",
            "walls-ag",
            "walls-bg",
            "windows",
            "roof",
            "grade",
        ] {
            assert!(!option(name).required, "{name}");
        }
        // A registered default would reach the handler as if typed and mask
        // the model's below-grade area.
        assert_eq!(option("walls-bg").default, None);
        assert_eq!(option("grade").default.as_deref(), Some("0"));
        assert!(option("epw").required);
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
        let (input, sources, _) =
            simulation_input(&context(&[("model", json.to_str().unwrap())])).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
        assert!(
            sources.iter().any(|s| s.contains("house.json")),
            "{sources:?}"
        );
    }

    #[test]
    fn given_window_directions_win_over_the_model() {
        let dir = folder("given-wins");
        let json = dir.join("house.json");
        write_model(&json, true);
        let (input, sources, _) = simulation_input(&context(&[
            ("model", json.to_str().unwrap()),
            ("window-directions", "N:1"),
        ]))
        .unwrap();
        assert_eq!(facings(&input.window_groups), vec![(0.0, 1.0)]);
        // the split line says so and does not credit the model
        let split = sources.last().unwrap();
        assert!(
            split.contains("as given") && !split.contains("house.json"),
            "{split}"
        );
    }

    #[test]
    fn an_ifc_model_uses_its_extraction() {
        let dir = folder("ifc");
        std::fs::write(dir.join("house.ifc"), "ISO-10303-21;").unwrap();
        write_envelope_model(&dir.join("house.json"), true, true, true);
        let ifc = dir.join("house.ifc");
        let (input, _, _) =
            simulation_input(&context(&[("model", ifc.to_str().unwrap())])).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
    }

    #[test]
    fn a_model_without_windows_gives_the_even_split() {
        let dir = folder("no-directions");
        let json = dir.join("blind.json");
        write_model(&json, false);
        let (input, sources, _) =
            simulation_input(&context(&[("model", json.to_str().unwrap())])).unwrap();
        assert_eq!(input.window_groups.len(), 4);
        assert!(
            sources.iter().any(|s| s.contains("even split")),
            "{sources:?}"
        );
    }

    #[test]
    fn an_ifc_that_cannot_be_extracted_is_an_error() {
        let dir = folder("unextracted");
        std::fs::write(dir.join("other.ifc"), "ISO-10303-21;").unwrap();
        let ifc = dir.join("other.ifc");
        let err = simulation_input(&context(&[("model", ifc.to_str().unwrap())]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not extracted"), "{err}");
    }

    // ─── Envelope areas from the model ──────────────────────────────────

    fn round1(x: f64) -> f64 {
        (x * 10.0).round() / 10.0
    }

    /// A format-2 model: storey L1 with a 120 m² plate and a 90 m² roof
    /// (each unless told otherwise); walls e (−1 to 3 m, 10 m², 3 m² of
    /// windows facing east) and s (0 to 3 m, 10 m², 1 m² facing south).
    fn write_envelope_model(path: &std::path::Path, storeys: bool, roof: bool, windows: bool) {
        let win = |area: f64| {
            if windows {
                serde_json::json!([{ "id": format!("w{area}"), "area": area }])
            } else {
                serde_json::json!([])
            }
        };
        let mut storey_list = Vec::new();
        let mut slabs = Vec::new();
        if storeys {
            storey_list.push(serde_json::json!({
                "id": "l1", "name": "L1", "elevation": 0.0, "floorArea": 120.0, "slabIds": ["p"]
            }));
            slabs.push(serde_json::json!({
                "id": "p", "name": "plate", "kind": "floor", "area": 120.0,
                "top": 0.0, "bottom": -0.2, "storey": "L1"
            }));
        }
        if roof {
            slabs.push(
                serde_json::json!({ "id": "r", "name": "roof", "kind": "roof", "area": 90.0 }),
            );
        }
        let model = serde_json::json!({
            "storeys": storey_list,
            "slabs": slabs,
            "walls": [
                { "id": "e", "orientation": { "x": 1.0, "y": 0.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "bottom": -1.0, "top": 3.0, "windows": win(3.0) },
                { "id": "s", "orientation": { "x": 0.0, "y": -1.0, "z": 0.0 }, "area": 10.0,
                  "isExternal": true, "bottom": 0.0, "top": 3.0, "windows": win(1.0) }
            ],
            "metadata": { "projectName": "p", "formatVersion": 2 }
        });
        std::fs::write(path, model.to_string()).unwrap();
    }

    /// An EPW and a model, plus `extra`: nothing else typed.
    fn model_context(json: &std::path::Path, extra: &[(&str, &str)]) -> TaskContext {
        let mut ctx = TaskContext::new();
        ctx.options.insert("epw".into(), "toronto.epw".into());
        ctx.options
            .insert("model".into(), json.to_str().unwrap().into());
        for (key, value) in extra {
            ctx.options.insert(key.to_string(), value.to_string());
        }
        ctx
    }

    #[test]
    fn the_run_is_named_after_the_model_and_the_weather_station() {
        let dir = folder("naming");
        let epw = dir.join("CAN_ON_Toronto.epw");
        std::fs::write(
            &epw,
            "LOCATION,Toronto.City-Univ.of.Toronto,ON,CAN,SRC-TMYx,715080,43.6658,-79.3953,-5.0,112.5\nDESIGN CONDITIONS,0\n",
        )
        .unwrap();
        assert_eq!(epw_city(&epw).as_deref(), Some("Toronto"));
        assert_eq!(epw_city(&dir.join("missing.epw")), None);

        let json = dir.join("Ifc4_ARC.json");
        write_envelope_model(&json, true, true, true);
        let mut ctx = model_context(&json, &[]);
        ctx.options
            .insert("epw".into(), epw.to_str().unwrap().into());
        let (input, _, _) = simulation_input(&ctx).unwrap();
        assert_eq!(input.name, "Ifc4_ARC in Toronto");

        // a typed name wins; without a model the default name takes the station
        ctx.options.insert("name".into(), "Tower B".into());
        assert_eq!(simulation_input(&ctx).unwrap().0.name, "Tower B");
        let mut bare = context(&[]);
        bare.options
            .insert("epw".into(), epw.to_str().unwrap().into());
        assert_eq!(
            simulation_input(&bare).unwrap().0.name,
            "makit_run in Toronto"
        );

        // an EPW that cannot be read leaves the station out
        let (input, _, _) = simulation_input(&model_context(&json, &[])).unwrap();
        assert_eq!(input.name, "Ifc4_ARC");
    }

    #[test]
    fn a_tui_run_with_the_registered_defaults_laid_over_still_derives_the_name() {
        // The TUI lays every registered default over a run's options, so a
        // default name would reach the handler as if typed.
        let mut reg = Registry::default();
        register_tasks(&mut reg);
        let dir = folder("tui-name");
        let epw = dir.join("CAN_ON_Toronto.epw");
        std::fs::write(&epw, "LOCATION,Toronto.City-Univ.of.Toronto,ON,CAN,SRC-TMYx,715080,43.6658,-79.3953,-5.0,112.5\n").unwrap();
        let json = dir.join("Ifc4_ARC.json");
        write_envelope_model(&json, true, true, true);
        let mut ctx = TaskContext::new();
        for option in &reg.actions["murb-simulate"].options {
            if let Some(default) = &option.default {
                ctx.options.insert(option.name.clone(), default.clone());
            }
        }
        ctx.options
            .insert("epw".into(), epw.to_str().unwrap().into());
        ctx.options
            .insert("model".into(), json.to_str().unwrap().into());
        let (input, _, _) = simulation_input(&ctx).unwrap();
        assert_eq!(input.name, "Ifc4_ARC in Toronto");
        // and the model's areas still come through the defaults
        assert_eq!(input.gfa, 120.0);
        assert_eq!(round1(input.area_walls_bg), 2.5);
    }

    #[test]
    fn the_report_goes_beside_the_results_file() {
        let dir = folder("report-dir");
        let json = dir.join("house.json");
        write_envelope_model(&json, true, true, true);
        let out = dir.join("out").join("run.json");
        let (input, _, _) =
            simulation_input(&model_context(&json, &[("output", out.to_str().unwrap())])).unwrap();
        assert_eq!(
            input.report_dir.as_deref(),
            Some(dir.join("out").to_str().unwrap())
        );
        // the default results file is in the working directory, so is the report
        let (input, _, _) = simulation_input(&model_context(&json, &[])).unwrap();
        assert_eq!(
            input.report_dir.as_deref().map(std::path::PathBuf::from),
            std::env::current_dir().ok()
        );
        // no results file, no report
        let (input, _, _) = simulation_input(&model_context(&json, &[("output", "")])).unwrap();
        assert_eq!(input.report_dir, None);
    }

    #[test]
    fn results_carry_the_report_path_when_the_runner_gives_one() {
        let base = r#"{"name": "h_TMY", "monthly_heating_demand": [], "monthly_cooling_demand": [],
            "monthly_heating_consumption": [], "monthly_cooling_consumption": [],
            "monthly_total_consumption": [], "monthly_ghg_emissions": [], "tedi_kwh_m2": 1.0,
            "teui_kwh_m2": 2.0, "ghgi_kg_m2": 3.0, "gfa": 4.0, "weather_file": "T""#;
        let old: MurbResults = serde_json::from_str(&format!("{base}}}")).unwrap();
        assert_eq!((old.report_path, old.report_error), (None, None));
        let new: MurbResults = serde_json::from_str(&format!(
            r#"{base}, "report_path": "/r/results/h_TMY/h_TMY.html", "report_error": null}}"#
        ))
        .unwrap();
        assert_eq!(
            new.report_path.as_deref(),
            Some("/r/results/h_TMY/h_TMY.html")
        );
    }

    #[test]
    fn the_model_fills_every_untyped_area() {
        let dir = folder("fills");
        let json = dir.join("house.json");
        write_envelope_model(&json, true, true, true);
        let (input, sources, envelope) = simulation_input(&model_context(&json, &[])).unwrap();
        assert_eq!(
            (input.gfa, input.area_roof, input.area_windows),
            (120.0, 90.0, 4.0)
        );
        assert_eq!(round1(input.area_walls_bg), 2.5);
        assert_eq!(round1(input.area_walls_ag), 17.5);
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
        assert!(
            sources
                .iter()
                .any(|s| s.starts_with("GFA") && s.contains("house.json")),
            "{sources:?}"
        );
        assert_eq!(envelope.map(|e| e.gfa), Some(120.0));
    }

    #[test]
    fn typed_areas_win_over_the_model() {
        let dir = folder("typed-wins");
        let json = dir.join("house.json");
        write_envelope_model(&json, true, true, true);
        let (input, sources, _) =
            simulation_input(&model_context(&json, &[("gfa", "500"), ("walls-bg", "7")])).unwrap();
        assert_eq!((input.gfa, input.area_walls_bg), (500.0, 7.0));
        assert_eq!(input.area_roof, 90.0);
        assert!(
            sources
                .iter()
                .any(|s| s.starts_with("GFA") && s.contains("typed")),
            "{sources:?}"
        );
    }

    #[test]
    fn grade_moves_the_split() {
        let dir = folder("grade");
        let json = dir.join("house.json");
        write_envelope_model(&json, true, true, true);
        // grade 1 m: wall e (-1 to 3) is half below it, wall s (0 to 3) a third
        let (input, _, _) = simulation_input(&model_context(&json, &[("grade", "1")])).unwrap();
        assert_eq!(round1(input.area_walls_bg), 8.3);
        assert_eq!(round1(input.area_walls_ag), 11.7);
        // a grade above every wall leaves no above-grade walls, which is a gap
        let err = simulation_input(&model_context(&json, &[("grade", "3")]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("walls-ag: the model"), "{err}");
    }

    #[test]
    fn without_a_model_missing_areas_name_their_option() {
        let mut ctx = TaskContext::new();
        ctx.options.insert("epw".into(), "toronto.epw".into());
        let err = simulation_input(&ctx).unwrap_err().to_string();
        assert!(err.contains("gfa: give --gfa or --model"), "{err}");
        // below-grade walls default to none without a model
        let (input, _, _) = simulation_input(&context(&[])).unwrap();
        assert_eq!(input.area_walls_bg, 0.0);
    }

    #[test]
    fn a_model_zero_is_a_gap_for_gfa_walls_and_roof_but_not_windows() {
        let dir = folder("zero");
        let json = dir.join("bare.json");
        write_envelope_model(&json, false, false, false);
        let err = simulation_input(&model_context(&json, &[]))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("gfa: the model") && err.contains("no storeys"),
            "{err}"
        );
        let err = simulation_input(&model_context(&json, &[("gfa", "100")]))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("roof: the model") && err.contains("no roof"),
            "{err}"
        );
        let (input, _, _) =
            simulation_input(&model_context(&json, &[("gfa", "100"), ("roof", "40")])).unwrap();
        assert_eq!(input.area_windows, 0.0);
        assert_eq!(round1(input.area_walls_bg), 2.5);
        assert_eq!(input.window_groups.len(), 4);
    }

    #[test]
    fn a_format_one_extraction_says_to_re_extract() {
        let dir = folder("format1");
        let json = dir.join("old.json");
        write_model(&json, true);
        let err = simulation_input(&model_context(&json, &[]))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("predates storeys") && err.contains("makit extract"),
            "{err}"
        );
        // with every area typed it still runs, taking only the window split
        let (input, _, _) =
            simulation_input(&context(&[("model", json.to_str().unwrap())])).unwrap();
        assert_eq!(
            facings(&input.window_groups),
            vec![(90.0, 0.75), (180.0, 0.25)]
        );
    }

    #[test]
    fn a_json_model_never_needs_python() {
        // A .json model loads directly: no interpreter is looked for, so this
        // holds with MAKIT_PYTHON unset and no .venv anywhere near the file.
        let dir = folder("json-only");
        let json = dir.join("house.json");
        write_envelope_model(&json, true, true, true);
        assert!(simulation_input(&model_context(&json, &[])).is_ok());
    }

    // ─── The report page's data ─────────────────────────────────────────

    #[test]
    fn an_even_split_needs_no_comparison_run() {
        let mut input = MurbInput {
            epw_path: "t.epw".into(),
            name: "house in Toronto".into(),
            province: "ON".into(),
            gfa: 100.0,
            area_walls_ag: 50.0,
            area_walls_bg: 0.0,
            area_windows: 10.0,
            area_roof: 40.0,
            u_walls_ag: 0.273,
            u_windows: 2.56,
            u_roof: 0.164,
            cop_htg: 0.85,
            cop_clg: 5.2,
            cop_dhw: 0.85,
            hrv_efficiency: 0.55,
            window_groups: default_window_groups(),
            report_dir: Some("/r".into()),
        };
        assert!(even_glass_input(&input).is_none());
        input.window_groups = parse_window_directions("SE:3,SW:1").unwrap();
        let even = even_glass_input(&input).expect("a comparison run");
        assert_eq!(
            facings(&even.window_groups),
            facings(&default_window_groups())
        );
        assert_eq!(even.area_windows, 10.0);
        assert_eq!(even.name, "house in Toronto (even glass)");
        assert_eq!(even.report_dir, None);
    }

    #[test]
    fn the_command_line_reproduces_the_run() {
        let mut ctx = TaskContext::new();
        for (k, v) in [("model", "house.json"), ("epw", "a b.epw"), ("grade", "1")] {
            ctx.options.insert(k.into(), v.into());
        }
        assert_eq!(
            command_line(&ctx),
            "makit exec murb analysis murb-simulate -o epw='a b.epw' -o grade=1 -o model=house.json"
        );
    }

    #[test]
    fn results_keep_the_run_context_for_the_page() {
        let json = r#"{
            "name": "h_TMY", "monthly_heating_demand": [], "monthly_cooling_demand": [],
            "monthly_heating_consumption": [], "monthly_cooling_consumption": [],
            "monthly_total_consumption": [], "monthly_ghg_emissions": [], "tedi_kwh_m2": 1.0,
            "teui_kwh_m2": 2.0, "ghgi_kg_m2": 3.0, "gfa": 4.0, "weather_file": "T"
        }"#;
        let old: MurbResults = serde_json::from_str(json).unwrap();
        assert!(old.input.is_none() && old.envelope.is_none() && old.even_glass.is_none());
        assert!(old.model.is_none() && old.command.is_none());
        // what the handler adds is written and read back
        let mut full = old.clone();
        full.even_glass = Some(EvenGlass {
            tedi_kwh_m2: 5.0,
            teui_kwh_m2: 6.0,
            ghgi_kg_m2: 7.0,
            monthly_heating_demand: vec![1.0; 12],
            monthly_cooling_demand: vec![2.0; 12],
            monthly_total_consumption: vec![3.0; 12],
        });
        full.command = Some("makit exec murb analysis murb-simulate -o epw=t.epw".into());
        full.model = Some("h.ifc".into());
        let text = serde_json::to_string(&full).unwrap();
        let back: MurbResults = serde_json::from_str(&text).unwrap();
        assert_eq!(back.even_glass, full.even_glass);
        assert_eq!(back.command, full.command);
        // absent fields are not written
        assert!(!serde_json::to_string(&old).unwrap().contains("even_glass"));
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
            report_dir: None,
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
            report_path: None,
            report_error: None,
            input: None,
            envelope: None,
            model: None,
            even_glass: None,
            command: None,
        };

        let report = format_report(&results);
        assert!(report.contains("TEDI"));
        assert!(report.contains("93.00"));
        assert!(report.contains("TORONTO"));
    }
}
