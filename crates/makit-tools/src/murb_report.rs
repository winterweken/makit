//! The report page `murb-simulate` writes beside its results: the glass on a
//! compass next to the three intensities, the year, where the heat goes, the
//! envelope, and how the numbers were made. Everything the page states is derived here, in
//! [`page_data`]; the page's own script only lays it out.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::murb::MurbResults;

/// The page: inline styles and script, the title and the data filled in.
const PAGE: &str = include_str!("murb_report.html");

/// Compass points clockwise from north, as abbreviations and words.
const COMPASS: [(&str, &str); 8] = [
    ("N", "north"),
    ("NE", "north-east"),
    ("E", "east"),
    ("SE", "south-east"),
    ("S", "south"),
    ("SW", "south-west"),
    ("W", "west"),
    ("NW", "north-west"),
];

/// makit's default U-values and plant figures, named as such in the notes.
const DEFAULT_U: (f64, f64, f64) = (0.273, 2.56, 0.164);

/// A storey's share of the floor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Level {
    pub name: String,
    pub area: f64,
}

/// Glass facing one way.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Facing {
    /// Degrees clockwise from north.
    pub azimuth: f64,
    /// Square metres.
    pub area: f64,
    /// Percent of the glass with a facing.
    pub share: u32,
    /// "SE"
    pub name: String,
    /// "south-east"
    pub word: String,
}

/// The glass on the compass.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Glass {
    /// Square metres with a facing.
    pub total: f64,
    /// Square metres in no wall (skylights, say): counted, not on the compass.
    pub unfaced: f64,
    /// Biggest first.
    pub facings: Vec<Facing>,
}

/// The intensities of the even-glass comparison run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Even {
    pub tedi: f64,
    pub teui: f64,
    pub ghgi: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Intensities {
    pub tedi: f64,
    pub teui: f64,
    pub ghgi: f64,
    pub even: Option<Even>,
}

/// Monthly series in MWh.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Monthly {
    pub heating: Vec<f64>,
    pub cooling: Vec<f64>,
    pub energy: Vec<f64>,
}

/// Annual sums in MWh.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Totals {
    pub heating: f64,
    pub cooling: f64,
    pub energy: f64,
    pub even_heating: Option<f64>,
    pub even_cooling: Option<f64>,
}

/// What wraps the floor, in square metres.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnvelopeData {
    pub roof: f64,
    pub glass: f64,
    pub walls_ag: f64,
    pub walls_bg: f64,
    pub floor: f64,
    /// Storeys with a floor plate; 0 when not known.
    pub levels: usize,
    /// Walls above grade + glass + roof.
    pub above_grade: f64,
    /// `above_grade` to floor.
    pub above_grade_ratio: f64,
    /// Percent of the above-grade wall that is glass.
    pub glass_share: u32,
    /// Whether the areas are known at all (old results files have none).
    pub known: bool,
}

/// Heat moving one way, per square metre of floor over the year.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Flow {
    pub name: String,
    /// kWh/m², one decimal.
    pub kwh_m2: f64,
}

/// Where the heat goes: lost while heating by each path, gained while
/// heating, gained while cooling by each source.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeatFlows {
    pub heating_losses: Vec<Flow>,
    pub heating_gains: Vec<Flow>,
    pub cooling_gains: Vec<Flow>,
    /// Why they do not add up to TEDI.
    pub note: String,
}

/// Everything the page states.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageData {
    pub title: String,
    pub subtitle: String,
    pub weather: String,
    /// One sentence under the compass, or empty.
    pub lede: String,
    pub gfa: f64,
    pub levels: Vec<Level>,
    pub glass: Glass,
    pub intensities: Intensities,
    pub monthly: Monthly,
    pub totals: Totals,
    pub envelope: EnvelopeData,
    /// None for results written before makit ran the model itself.
    pub heat_flows: Option<HeatFlows>,
    pub notes: Vec<String>,
    pub command: Option<String>,
}

fn round(x: f64, decimals: i32) -> f64 {
    let k = 10f64.powi(decimals);
    (x * k).round() / k
}

fn mwh(kwh: &[f64]) -> Vec<f64> {
    kwh.iter().map(|v| round(v / 1000.0, 2)).collect()
}

fn total_mwh(kwh: &[f64]) -> f64 {
    round(kwh.iter().fold(0.0, |s, v| s + v) / 1000.0, 2)
}

/// The compass point nearest `azimuth`.
fn compass(azimuth: f64) -> (&'static str, &'static str) {
    let slot = ((azimuth.rem_euclid(360.0) + 22.5) / 45.0).floor() as usize % 8;
    COMPASS[slot]
}

fn facings(pairs: &[(f64, f64)]) -> Vec<Facing> {
    let total: f64 = pairs.iter().map(|(_, a)| a).fold(0.0, |s, v| s + v);
    let mut facings: Vec<Facing> = pairs
        .iter()
        .filter(|(_, area)| *area > 0.0)
        .map(|&(azimuth, area)| {
            let (name, word) = compass(azimuth);
            Facing {
                azimuth,
                area: round(area, 1),
                share: if total > 0.0 {
                    (area / total * 100.0).round() as u32
                } else {
                    0
                },
                name: name.to_string(),
                word: word.to_string(),
            }
        })
        .collect();
    facings.sort_by(|a, b| {
        b.area
            .partial_cmp(&a.area)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    facings
}

/// Whether two facings lists describe the same split: the same directions
/// (within half a degree) with the same shares (within a percent).
fn same_split(a: &[Facing], b: &[Facing]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a: Vec<&Facing> = a.iter().collect();
    let mut b: Vec<&Facing> = b.iter().collect();
    let by_azimuth = |x: &&Facing, y: &&Facing| {
        x.azimuth
            .partial_cmp(&y.azimuth)
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    a.sort_by(by_azimuth);
    b.sort_by(by_azimuth);
    a.iter()
        .zip(&b)
        .all(|(x, y)| (x.azimuth - y.azimuth).abs() <= 0.5 && x.share.abs_diff(y.share) <= 1)
}

/// The run's name without murb's weather suffix.
fn title_of(results: &MurbResults) -> String {
    let name = results.name.trim();
    for suffix in ["_TMY", "_ISD"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem.to_string();
        }
    }
    name.to_string()
}

/// Why the heat flows do not add up to TEDI.
const HEAT_FLOW_NOTE: &str = "Heating demand is worked out month by month and never goes below zero, so in mild months the gains outweigh the losses and the surplus is not carried over: these figures do not add up to the TEDI above.";

/// The year's heat flows per square metre of floor, from the results'
/// breakdown; none without one, or without a floor area.
fn heat_flows(results: &MurbResults) -> Option<HeatFlows> {
    let b = results.breakdown.as_ref()?;
    if results.gfa <= 0.0 {
        return None;
    }
    let flow = |name: &str, series: &[f64]| Flow {
        name: name.to_string(),
        kwh_m2: round(series.iter().sum::<f64>() / results.gfa, 1),
    };
    Some(HeatFlows {
        heating_losses: vec![
            flow("Transmission", &b.transmission_heat_losses),
            flow("Ventilation", &b.ventilation_heat_losses),
            flow("Infiltration", &b.infiltration_heat_losses),
        ],
        heating_gains: vec![
            flow("Internal gains", &b.internal_gains_during_htg),
            flow("Solar gains", &b.solar_gains_during_htg),
        ],
        cooling_gains: vec![
            flow("Transmission", &b.transmission_heat_gains),
            flow("Ventilation", &b.ventilation_heat_gains),
            flow("Infiltration", &b.infiltration_heat_gains),
            flow("Internal gains", &b.internal_gains_during_clg),
            flow("Solar gains", &b.solar_gains_during_clg),
        ],
        note: HEAT_FLOW_NOTE.to_string(),
    })
}

/// Everything the page states, derived from the results.
pub fn page_data(results: &MurbResults) -> PageData {
    let title = title_of(results);
    let envelope = results.envelope.as_ref();
    let input = results.input.as_ref();

    let levels: Vec<Level> = envelope
        .map(|e| {
            e.storeys
                .iter()
                .filter(|s| s.plates > 0 && s.area > 0.0)
                .map(|s| Level {
                    name: s.name.clone(),
                    area: round(s.area, 1),
                })
                .collect()
        })
        .unwrap_or_default();

    // The split the simulation ran with: the model's facings when the
    // window groups follow them, else the groups as typed.
    let model_glass = envelope.map(|e| {
        let facings = facings(&e.windows_by_facing);
        let total: f64 = facings.iter().map(|f| f.area).fold(0.0, |s, v| s + v);
        Glass {
            total: round(total, 1),
            unfaced: round((e.windows - total).max(0.0), 1),
            facings,
        }
    });
    let typed_glass = input.map(|i| {
        let pairs: Vec<(f64, f64)> = i
            .window_groups
            .iter()
            .map(|g| (g.window_azimuth, g.pct_window_area * i.area_windows))
            .collect();
        Glass {
            total: round(i.area_windows, 1),
            unfaced: 0.0,
            facings: facings(&pairs),
        }
    });
    let split_as_given = match (&model_glass, &typed_glass) {
        (Some(model), Some(typed)) => !same_split(&model.facings, &typed.facings),
        _ => false,
    };
    let glass = match (model_glass, typed_glass) {
        (Some(model), _) if !split_as_given => model,
        (_, Some(typed)) => typed,
        (Some(model), None) => model,
        (None, None) => Glass::default(),
    };

    let intensities = Intensities {
        tedi: results.tedi_kwh_m2,
        teui: results.teui_kwh_m2,
        ghgi: results.ghgi_kg_m2,
        even: results.even_glass.as_ref().map(|e| Even {
            tedi: e.tedi_kwh_m2,
            teui: e.teui_kwh_m2,
            ghgi: e.ghgi_kg_m2,
        }),
    };

    let monthly = Monthly {
        heating: mwh(&results.monthly_heating_demand),
        cooling: mwh(&results.monthly_cooling_demand),
        energy: mwh(&results.monthly_total_consumption),
    };
    let totals = Totals {
        heating: total_mwh(&results.monthly_heating_demand),
        cooling: total_mwh(&results.monthly_cooling_demand),
        energy: total_mwh(&results.monthly_total_consumption),
        even_heating: results
            .even_glass
            .as_ref()
            .map(|e| total_mwh(&e.monthly_heating_demand)),
        even_cooling: results
            .even_glass
            .as_ref()
            .map(|e| total_mwh(&e.monthly_cooling_demand)),
    };

    // The areas the simulation ran with (typed values win over the model);
    // the model keeps the storeys and the provenance.
    let (roof, glass_area, walls_ag, walls_bg, known) = match (input, envelope) {
        (Some(i), _) => (
            i.area_roof,
            i.area_windows,
            i.area_walls_ag,
            i.area_walls_bg,
            true,
        ),
        (None, Some(e)) => (e.roof, e.windows, e.walls_ag, e.walls_bg, true),
        (None, None) => (0.0, 0.0, 0.0, 0.0, false),
    };
    let floor = results.gfa;
    let above_grade = walls_ag + glass_area + roof;
    let envelope_data = EnvelopeData {
        roof: round(roof, 1),
        glass: round(glass_area, 1),
        walls_ag: round(walls_ag, 1),
        walls_bg: round(walls_bg, 1),
        floor: round(floor, 1),
        levels: levels.len(),
        above_grade: round(above_grade, 1),
        above_grade_ratio: if floor > 0.0 {
            round(above_grade / floor, 1)
        } else {
            0.0
        },
        glass_share: if walls_ag + glass_area > 0.0 {
            (glass_area / (walls_ag + glass_area) * 100.0).round() as u32
        } else {
            0
        },
        known,
    };

    let weather = results.weather_file.clone();
    let where_from = match (envelope, input) {
        (Some(_), _) => "Every area below was measured from the IFC model.",
        (None, Some(_)) => "The areas below are as typed.",
        (None, None) => "The areas were not recorded with these results.",
    };
    let subtitle = if levels.is_empty() {
        format!(
            "A building of {floor:.1} m² of floor, run through the monthly MURB heat balance with {weather} weather. {where_from}"
        )
    } else {
        format!(
            "A model with {} storeys and {floor:.1} m² of floor, run through the monthly MURB heat balance with {weather} weather. {where_from}",
            levels.len()
        )
    };

    let lede = match (glass.facings.first(), &intensities.even) {
        (Some(top), Some(even)) if even.tedi > 0.0 => {
            let change = (even.tedi - intensities.tedi) / even.tedi * 100.0;
            let (pct, more_or_less) = if change >= 0.0 {
                (change, "less")
            } else {
                (-change, "more")
            };
            format!(
                "{}% of the glass faces {}: {:.0}% {more_or_less} heating need than the same building with its glass spread evenly.",
                top.share, top.word, pct
            )
        }
        (Some(top), None) => format!("{}% of the glass faces {}.", top.share, top.word),
        _ => String::new(),
    };

    let mut notes = Vec::new();
    match (envelope, input) {
        (Some(e), _) => {
            let plates: Vec<String> = levels
                .iter()
                .map(|l| format!("{} {:.1} m²", l.name, l.area))
                .collect();
            let mut floor_note = format!(
                "Floor area is the footprint union of each storey's floor plates: {}.",
                if plates.is_empty() {
                    "none found".to_string()
                } else {
                    plates.join(", ")
                }
            );
            if !e.left_out.is_empty() {
                // the biggest first, the rest counted: a Revit model can
                // export dozens of pile stubs as slabs
                let mut left: Vec<&(String, f64)> = e.left_out.iter().collect();
                left.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                let total: f64 = left.iter().map(|(_, a)| a).fold(0.0, |s, v| s + v);
                let names: Vec<String> = left
                    .iter()
                    .take(3)
                    .map(|(name, area)| format!("{name} ({area:.1} m²)"))
                    .collect();
                let more = left.len().saturating_sub(3);
                floor_note.push_str(&format!(
                    " Left out, at no storey's level: {}{}.",
                    names.join(", "),
                    if more > 0 {
                        format!(" and {more} more, {total:.1} m² in all")
                    } else {
                        String::new()
                    }
                ));
            }
            if e.empty_storeys > 0 {
                floor_note.push_str(&format!(
                    " {} storey(s) have no plate at their level.",
                    e.empty_storeys
                ));
            }
            notes.push(floor_note);
            let mut glass_note = "Glass is the windows set in exterior walls plus curtain-wall glass, split by the wall's facing.".to_string();
            if glass.unfaced > 0.05 {
                glass_note.push_str(&format!(
                    " {:.1} m² of windows in no wall (skylights, say) count in the area but not on the compass.",
                    glass.unfaced
                ));
            }
            notes.push(glass_note);
            if split_as_given {
                let model_facings: Vec<String> = facings(&e.windows_by_facing)
                    .iter()
                    .map(|f| format!("{} {}%", f.word, f.share))
                    .collect();
                notes.push(format!(
                    "Glass split as given: {}; the model's windows face {}.",
                    glass
                        .facings
                        .iter()
                        .map(|f| format!("{} {}%", f.word, f.share))
                        .collect::<Vec<_>>()
                        .join(", "),
                    if model_facings.is_empty() {
                        "nowhere".to_string()
                    } else {
                        model_facings.join(", ")
                    }
                ));
            }
            notes.push(format!(
                "Walls are net of their openings and split at grade {:.1} m: {:.1} m² above, {:.1} m² below.",
                e.grade, e.walls_ag, e.walls_bg
            ));
            if let Some(i) = input {
                let overrides: Vec<String> = [
                    ("floor", i.gfa, e.gfa),
                    ("walls above grade", i.area_walls_ag, e.walls_ag),
                    ("walls below grade", i.area_walls_bg, e.walls_bg),
                    ("windows", i.area_windows, e.windows),
                    ("roof", i.area_roof, e.roof),
                ]
                .iter()
                .filter(|(_, typed, model)| (typed - model).abs() > 0.05)
                .map(|(what, typed, model)| format!("{what} {typed:.1} m² (model {model:.1} m²)"))
                .collect();
                if !overrides.is_empty() {
                    notes.push(format!("Typed over the model: {}.", overrides.join(", ")));
                }
            }
        }
        (None, Some(_)) => notes.push(
            "The floor, wall, window and roof areas are as typed; no model was used.".to_string(),
        ),
        (None, None) => notes.push(
            "The areas were not recorded with these results; only the floor area the simulation used is known.".to_string(),
        ),
    }
    if let Some(i) = input {
        let defaults = (i.u_walls_ag, i.u_windows, i.u_roof) == DEFAULT_U;
        notes.push(format!(
            "U-values: walls {}, windows {}, roof {} W/m²K{}; heat recovery {:.0}%; heating COP {}, cooling COP {}.",
            i.u_walls_ag,
            i.u_windows,
            i.u_roof,
            if defaults { " (makit's defaults)" } else { "" },
            i.hrv_efficiency * 100.0,
            i.cop_htg,
            i.cop_clg
        ));
    }
    notes.push(
        "The energy model, ported from murb_energy_tool, is written for multi-unit residential buildings; for other building types treat these results as indicative.".to_string(),
    );

    PageData {
        title,
        subtitle,
        weather,
        lede,
        gfa: round(floor, 1),
        levels,
        glass,
        intensities,
        monthly,
        totals,
        envelope: envelope_data,
        heat_flows: heat_flows(results),
        notes,
        command: results.command.clone(),
    }
}

/// `<`, `>`, `&` and quotes as entities.
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The page for `results`: one self-contained HTML document.
pub fn render_html(results: &MurbResults) -> String {
    let data = page_data(results);
    // Inside a script element only "</" can end it; JSON allows "\/".
    let json = serde_json::to_string(&data)
        .unwrap_or_else(|_| "{}".to_string())
        .replace("</", "<\\/");
    let title = escape_html(&data.title);
    // One pass over the template, so a title or value never gets re-read as a placeholder.
    let mut out = String::with_capacity(PAGE.len() + json.len());
    let mut rest = PAGE;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        match &after[..end] {
            "TITLE" => out.push_str(&title),
            "DATA" => out.push_str(&json),
            other => {
                out.push_str("{{");
                out.push_str(other);
                out.push_str("}}");
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Where the page goes for a results file: `<stem>.html` beside it.
pub fn page_path_for(results_path: &Path) -> PathBuf {
    results_path.with_extension("html")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::murb::{Breakdown, EvenGlass, MurbInput, MurbResults, WindowGroupInput};
    use std::path::{Path, PathBuf};

    fn round1(x: f64) -> f64 {
        (x * 10.0).round() / 10.0
    }

    fn group(azimuth: f64, share: f64) -> WindowGroupInput {
        WindowGroupInput {
            pct_window_area: share,
            window_azimuth: azimuth,
            shgc: 0.4,
            shading: 0.0,
        }
    }

    /// The Revit sample house in Toronto, as murb-simulate saves it.
    fn arc_results() -> MurbResults {
        let input = MurbInput {
            epw_path: "examples/EPW/toronto.epw".into(),
            name: "Ifc4_ARC in Toronto".into(),
            province: "ON".into(),
            gfa: 718.8,
            area_walls_ag: 532.5,
            area_walls_bg: 174.7,
            area_windows: 197.3,
            area_roof: 244.3,
            u_walls_ag: 0.273,
            u_windows: 2.56,
            u_roof: 0.164,
            cop_htg: 0.85,
            cop_clg: 5.2,
            cop_dhw: 0.85,
            hrv_efficiency: 0.55,
            window_groups: vec![group(143.0, 0.67), group(233.0, 0.21), group(53.0, 0.12)],
        };
        let envelope: crate::building_model::Envelope = serde_json::from_value(serde_json::json!({
            "grade": 0.0, "gfa": 718.8,
            "storeys": [
                { "name": "Level 1", "elevation": 0.0, "area": 144.4, "plates": 2 },
                { "name": "Level 2", "elevation": 3.0, "area": 259.1, "plates": 5 }
            ],
            "empty_storeys": 1, "walls_ag": 532.5, "walls_bg": 174.7, "walls_without_height": 0,
            "windows": 197.3, "windows_by_facing": [[53.0, 22.4], [143.0, 128.9], [233.0, 40.6]],
            "roof": 244.3, "roofs": [["SG Metal Panels", 173.6], ["Warm Roof", 70.8]],
            "left_out": [["Pad:Pad 2", 287.3]], "format_version": 2
        }))
        .unwrap();
        MurbResults {
            name: "Ifc4_ARC in Toronto_TMY".into(),
            monthly_heating_demand: vec![
                10290.0, 9100.0, 7580.0, 4150.0, 1390.0, 30.0, 260.0, 270.0, 90.0, 2170.0, 5810.0,
                9550.0,
            ],
            monthly_cooling_demand: vec![
                200.0, 340.0, 710.0, 1050.0, 1830.0, 3330.0, 5910.0, 6160.0, 2820.0, 950.0, 450.0,
                130.0,
            ],
            monthly_heating_consumption: vec![1.0; 12],
            monthly_cooling_consumption: vec![0.5; 12],
            monthly_total_consumption: vec![
                14930.0, 13290.0, 11840.0, 7780.0, 4770.0, 3370.0, 4230.0, 4290.0, 3340.0, 5520.0,
                9620.0, 14050.0,
            ],
            monthly_ghg_emissions: vec![0.1; 12],
            tedi_kwh_m2: 86.76,
            teui_kwh_m2: 207.76,
            ghgi_kg_m2: 31.67,
            gfa: 718.8,
            weather_file: "Toronto.City-Univ.of.Toronto".into(),
            breakdown: None,
            input: Some(input),
            envelope: Some(envelope),
            model: Some("examples/IFC/Ifc4_ARC.ifc".into()),
            even_glass: Some(EvenGlass {
                tedi_kwh_m2: 95.1,
                teui_kwh_m2: 215.4,
                ghgi_kg_m2: 33.0,
                monthly_heating_demand: vec![5700.0; 12],
                monthly_cooling_demand: vec![1450.0; 12],
                monthly_total_consumption: vec![9000.0; 12],
            }),
            command: Some(
                "makit exec murb analysis murb-simulate -o epw=examples/EPW/toronto.epw -o model=examples/IFC/Ifc4_ARC.ifc"
                    .into(),
            ),
        }
    }

    #[test]
    fn the_page_data_tells_the_story_of_the_run() {
        let data = page_data(&arc_results());
        assert_eq!(data.title, "Ifc4_ARC in Toronto");
        assert_eq!(data.weather, "Toronto.City-Univ.of.Toronto");
        assert!(
            data.subtitle.contains("2 storeys") && data.subtitle.contains("718.8"),
            "{}",
            data.subtitle
        );
        assert_eq!(data.levels.len(), 2);

        // glass by facing, biggest first, with its share and compass name
        assert_eq!(round1(data.glass.total), 191.9);
        assert_eq!(round1(data.glass.unfaced), 5.4);
        let first = &data.glass.facings[0];
        assert_eq!(
            (first.azimuth, first.name.as_str(), first.share),
            (143.0, "SE", 67)
        );
        assert_eq!(data.glass.facings[2].name, "NE");

        // intensities against the even-glass run
        let even = data.intensities.even.as_ref().unwrap();
        assert_eq!(round1(even.tedi), 95.1);
        assert_eq!(round1(data.intensities.tedi - even.tedi), -8.3);
        assert!(
            data.lede.contains("67%")
                && data.lede.contains("south-east")
                && data.lede.contains("9%"),
            "{}",
            data.lede
        );

        // the year in MWh
        assert_eq!(data.monthly.heating[0], 10.29);
        assert_eq!(data.monthly.cooling[7], 6.16);
        assert_eq!(round1(data.totals.heating), 50.7);
        assert_eq!(round1(data.totals.energy), 97.0);
        assert_eq!(data.totals.even_heating.map(round1), Some(68.4));

        // the envelope and its ratios
        assert_eq!(data.envelope.levels, 2);
        assert_eq!(round1(data.envelope.above_grade), 974.1);
        assert_eq!(round1(data.envelope.above_grade_ratio), 1.4);
        assert_eq!(data.envelope.glass_share, 27);

        assert!(
            data.notes.iter().any(|n| n.contains("Pad:Pad 2")),
            "{:?}",
            data.notes
        );
        assert!(
            data.notes.iter().any(|n| n.contains("0.273")),
            "{:?}",
            data.notes
        );
        assert!(
            data.notes.iter().any(|n| n.contains("grade 0.0 m")),
            "{:?}",
            data.notes
        );
        assert!(data
            .command
            .as_deref()
            .unwrap()
            .starts_with("makit exec murb"));
    }

    /// The JSON the page embeds.
    #[test]
    fn the_page_shows_where_the_heat_goes() {
        let every = vec![100.0; 12];
        let mut results = arc_results();
        results.breakdown = Some(Breakdown {
            transmission_heat_losses: every.clone(),
            transmission_heat_gains: every.clone(),
            ventilation_heat_losses: every.clone(),
            ventilation_heat_gains: every.clone(),
            infiltration_heat_losses: every.clone(),
            infiltration_heat_gains: every.clone(),
            internal_gains_during_htg: every.clone(),
            internal_gains_during_clg: every.clone(),
            solar_gains_during_htg: vec![50.0; 12],
            solar_gains_during_clg: every.clone(),
            lighting_consumption: every.clone(),
            dhw_htg_consumption: every.clone(),
            plug_loads_consumption: every.clone(),
            electricity_consumption: every.clone(),
            gas_consumption: every,
        });
        let g = results.gfa;
        let flows = page_data(&results).heat_flows.expect("heat flows");
        let names = |list: &[Flow]| list.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(&flows.heating_losses),
            ["Transmission", "Ventilation", "Infiltration"]
        );
        assert!(flows
            .heating_losses
            .iter()
            .all(|f| f.kwh_m2 == round1(1200.0 / g)));
        assert_eq!(
            names(&flows.heating_gains),
            ["Internal gains", "Solar gains"]
        );
        assert_eq!(flows.heating_gains[1].kwh_m2, round1(600.0 / g));
        assert_eq!(
            names(&flows.cooling_gains),
            [
                "Transmission",
                "Ventilation",
                "Infiltration",
                "Internal gains",
                "Solar gains"
            ]
        );
        assert_eq!(
            flows.note,
            "Heating demand is worked out month by month and never goes below zero, so in mild months the gains outweigh the losses and the surplus is not carried over: these figures do not add up to the TEDI above."
        );
        let html = render_html(&results);
        assert!(html.contains(r#"aria-labelledby="heat-h""#));
        assert_eq!(
            embedded_data(&html)["heat_flows"]["heating_gains"][0]["name"],
            "Internal gains"
        );
    }

    #[test]
    fn old_results_have_no_heat_flows() {
        let results = arc_results();
        assert!(results.breakdown.is_none());
        assert_eq!(page_data(&results).heat_flows, None);
        assert!(embedded_data(&render_html(&results))["heat_flows"].is_null());
    }

    #[test]
    fn the_caveat_names_the_engine_makit_carries() {
        let notes = page_data(&arc_results()).notes;
        assert!(notes.iter().any(|n| n
            == "The energy model, ported from murb_energy_tool, is written for multi-unit residential buildings; for other building types treat these results as indicative."));
        assert!(!notes
            .iter()
            .any(|n| n.starts_with("murb_energy_tool is written")));
    }

    fn embedded_data(html: &str) -> serde_json::Value {
        let marker = r#"<script id="makit-data" type="application/json">"#;
        let start = html.find(marker).expect("data script") + marker.len();
        let end = html[start..].find("</script>").unwrap() + start;
        serde_json::from_str(&html[start..end]).expect("embedded JSON parses")
    }

    #[test]
    fn the_page_is_self_contained_and_carries_its_data() {
        let html = render_html(&arc_results());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<title>Ifc4_ARC in Toronto</title>"));
        assert!(!html.contains("<link "), "no external stylesheets");
        assert!(!html.contains("src=\"http"), "no external scripts");
        let data = embedded_data(&html);
        assert_eq!(data["title"], "Ifc4_ARC in Toronto");
        assert_eq!(data["monthly"]["heating"].as_array().unwrap().len(), 12);
        assert_eq!(data["glass"]["facings"][0]["name"], "SE");
    }

    #[test]
    fn a_name_with_markup_cannot_break_the_page() {
        let mut results = arc_results();
        results.name = "<script>alert(1)</script> & co_TMY".into();
        let html = render_html(&results);
        // the title is text, and the data block cannot be closed early
        assert!(html.contains("<title>&lt;script&gt;alert(1)&lt;/script&gt; &amp; co</title>"));
        assert_eq!(
            html.matches("</script>").count(),
            2,
            "the data block and the page script"
        );
        assert!(html.contains(r#"<\/script>"#));
        assert_eq!(
            embedded_data(&html)["title"],
            "<script>alert(1)</script> & co"
        );
    }

    #[test]
    fn old_results_without_a_model_still_make_a_page() {
        let mut results = arc_results();
        results.input = None;
        results.envelope = None;
        results.even_glass = None;
        results.command = None;
        results.model = None;
        let data = page_data(&results);
        assert!(data.glass.facings.is_empty());
        assert!(data.intensities.even.is_none());
        assert_eq!(data.envelope.floor, 718.8);
        assert_eq!(data.envelope.levels, 0);
        assert!(
            data.notes.iter().any(|n| n.contains("not recorded")),
            "{:?}",
            data.notes
        );
        assert!(render_html(&results).contains("718.8"));

        // typed areas without a model: the compass comes from the split
        let mut typed = arc_results();
        typed.envelope = None;
        typed.model = None;
        let data = page_data(&typed);
        assert_eq!(data.glass.facings.len(), 3);
        assert_eq!(round1(data.glass.facings[0].area), round1(197.3 * 0.67));
        assert_eq!(data.envelope.levels, 0);
        assert!(
            data.notes.iter().any(|n| n.contains("as typed")),
            "{:?}",
            data.notes
        );
    }

    #[test]
    fn long_left_out_lists_are_summarised_biggest_first() {
        let mut results = arc_results();
        let envelope = results.envelope.as_mut().unwrap();
        envelope.left_out = vec![
            ("stub a".into(), 0.1),
            ("Pad:Pad 2".into(), 287.3),
            ("stub b".into(), 0.1),
            ("stub c".into(), 0.1),
            ("stub d".into(), 0.1),
        ];
        let note = page_data(&results)
            .notes
            .into_iter()
            .find(|n| n.contains("Left out"))
            .unwrap();
        assert!(
            note.contains("Pad:Pad 2 (287.3 m²), stub a (0.1 m²), stub b (0.1 m²) and 2 more, 287.7 m² in all"),
            "{note}"
        );
    }

    #[test]
    fn typed_areas_over_the_model_are_what_the_page_shows() {
        let mut results = arc_results();
        results.input.as_mut().unwrap().area_roof = 300.0;
        results.input.as_mut().unwrap().area_walls_bg = 0.0;
        let data = page_data(&results);
        // the envelope the simulation ran with, not the one the model measured
        assert_eq!(data.envelope.roof, 300.0);
        assert_eq!(data.envelope.walls_bg, 0.0);
        assert_eq!(
            round1(data.envelope.above_grade),
            round1(532.5 + 197.3 + 300.0)
        );
        // the model still gives the storeys and the provenance
        assert_eq!(data.envelope.levels, 2);
        let note = data
            .notes
            .iter()
            .find(|n| n.contains("Typed over the model"))
            .expect("an override note");
        assert!(note.contains("roof 300.0 m² (model 244.3 m²)"), "{note}");
        assert!(
            note.contains("walls below grade 0.0 m² (model 174.7 m²)"),
            "{note}"
        );
        assert!(!note.contains("windows"), "{note}");
    }

    #[test]
    fn typed_window_directions_drive_the_compass() {
        let mut results = arc_results();
        results.input.as_mut().unwrap().window_groups = vec![group(0.0, 1.0)];
        let data = page_data(&results);
        assert_eq!(data.glass.facings.len(), 1);
        let north = &data.glass.facings[0];
        assert_eq!(
            (north.azimuth, north.name.as_str(), north.share),
            (0.0, "N", 100)
        );
        assert_eq!(north.area, 197.3);
        assert!(data.lede.contains("north"), "{}", data.lede);
        let note = data
            .notes
            .iter()
            .find(|n| n.contains("split as given"))
            .expect("a note on the given split");
        assert!(note.contains("south-east"), "{note}");
    }

    #[test]
    fn the_page_goes_beside_the_results() {
        assert_eq!(
            page_path_for(Path::new("out/murb_results.json")),
            PathBuf::from("out/murb_results.html")
        );
        assert_eq!(page_path_for(Path::new("run")), PathBuf::from("run.html"));
    }
}
