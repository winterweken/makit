//! The MURB energy model: a monthly heat balance (TEDI, TEUI, GHGI) worked
//! out from hourly EPW weather, run in-process.
//!
//! This is a port of `murb_energy_tool` from github.com/winterweken/murb at
//! commit a1d9e81 (its pure-Python port, merged into murb `main` as 83cf3b8),
//! module for module, keeping murb's order of operations so the numbers match
//! to the last few bits:
//!
//! | here | murb |
//! |---|---|
//! | `solar.rs` | `solarpos.py`, `solargains.py` |
//! | `weather.rs` | `epw.py`, the weather half of `utilities.py` |
//! | `heat_balance.rs` | `heatbalance.py` |
//! | `end_uses.rs` | `models.py` |
//! | `constants.rs` | `static.py` |
//!
//! The tests check every function against what murb's Python computes on a
//! synthetic weather file (`testdata/parity.json`, made by
//! `testdata/generate_parity.py`). This module depends on nothing else in
//! makit.

use std::path::PathBuf;

pub mod constants;
pub mod end_uses;
pub mod heat_balance;
pub mod solar;
pub mod weather;

#[cfg(test)]
pub(crate) mod test_support;

/// A value for each month, January first.
pub type Monthly = [f64; 12];

/// Everything a run takes, named as murb's `Run` names its arguments.
/// `Default` gives murb's defaults (the NECB and ASHRAE assumptions in
/// [`constants`]); the five areas, the province and the window groups are the
/// building's own.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize), serde(default))]
pub struct Inputs {
    /// Two letters: see [`constants::PROVINCES`].
    pub province: String,
    /// Gross floor area, m2.
    pub gfa: f64,
    /// Areas of the walls above and below grade, the windows and the roof, m2.
    pub area_walls_ag: f64,
    pub area_walls_bg: f64,
    pub area_windows: f64,
    pub area_roof: f64,
    pub window_groups: Vec<WindowGroup>,
    /// Slabs on grade, m2, and the perimeter of exposed slabs, m.
    pub area_sog: f64,
    pub perim_exp: f64,
    /// Heating and cooling set-points, °C.
    pub setpoint_htg: f64,
    pub setpoint_clg: f64,
    /// Seasonal COPs of the heating, cooling and hot water plants. Below 0.99
    /// the plant burns gas; otherwise it runs on electricity.
    pub cop_htg: f64,
    pub cop_clg: f64,
    pub cop_dhw: f64,
    /// Sensible heat recovery effectiveness, 0 to 1.
    pub hrv_efficiency: f64,
    /// Surface-to-surface U-values, W/m2K, and the F-factor of exposed slabs, W/mK.
    pub u_walls_ag: f64,
    pub u_walls_bg: f64,
    pub f_factor: f64,
    pub u_windows: f64,
    pub u_roof: f64,
    /// Air leakage at 75 Pa, L/s per m2 of exterior area.
    pub test_leakage: f64,
    /// Share of the floor area that is cooled, 0 to 1.
    pub clg_pct: f64,
    /// m2 per occupant.
    pub occupant_density: f64,
    /// Peak plug load and lighting power density, W/m2.
    pub peak_receptacle_load: f64,
    pub lpd: f64,
    /// Effective shares of the year the building is occupied, lit and using plug loads.
    pub fraction_occupied: f64,
    pub fraction_lighting: f64,
    pub fraction_receptacle_load: f64,
    /// W per occupant.
    pub service_water_heating_load: f64,
    /// Outdoor air per person (L/s) and per floor area (L/s m2).
    pub people_outdoor_air_rate: f64,
    pub area_outdoor_air_rate: f64,
    pub mass_level: MassLevel,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            province: "ON".to_string(),
            gfa: 0.0,
            area_walls_ag: 0.0,
            area_walls_bg: 0.0,
            area_windows: 0.0,
            area_roof: 0.0,
            window_groups: Vec::new(),
            area_sog: 0.0,
            perim_exp: 0.0,
            setpoint_htg: 21.0,
            setpoint_clg: 24.0,
            cop_htg: 0.85,
            cop_clg: 5.2,
            cop_dhw: 0.85,
            hrv_efficiency: 0.55,
            u_walls_ag: 0.273,
            u_walls_bg: 0.379,
            f_factor: 0.93,
            u_windows: 2.56,
            u_roof: 0.164,
            test_leakage: 2.0,
            clg_pct: 1.0,
            occupant_density: constants::OCCUPANT_DENSITY,
            peak_receptacle_load: constants::PEAK_RECEPTACLE_LOAD,
            lpd: constants::LPD,
            fraction_occupied: constants::FRACTION_OCCUPIED,
            fraction_lighting: constants::FRACTION_LIGHTING,
            fraction_receptacle_load: constants::FRACTION_RECEPTACLE_LOAD,
            service_water_heating_load: constants::SERVICE_WATER_HEATING_LOAD,
            people_outdoor_air_rate: constants::PEOPLE_OUTDOOR_AIR_RATE,
            area_outdoor_air_rate: constants::AREA_OUTDOOR_AIR_RATE,
            mass_level: MassLevel::Medium,
        }
    }
}

/// Glass facing one way.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Deserialize), serde(default))]
pub struct WindowGroup {
    /// Share of the building's window area, 0 to 1.
    pub pct_window_area: f64,
    /// Degrees clockwise from north.
    pub window_azimuth: f64,
    /// Solar heat gain coefficient.
    pub shgc: f64,
    /// Share of the sun the shading keeps out, 0 to 1.
    pub shading: f64,
}

impl Default for WindowGroup {
    fn default() -> Self {
        Self {
            pct_window_area: 0.0,
            window_azimuth: 0.0,
            shgc: 0.4,
            shading: 0.0,
        }
    }
}

/// How much thermal mass the building has, which sets how much of the sun's
/// heat is useful (murb's `mass_level`; murb reads anything other than
/// `low` or `medium` as high).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "lowercase"))]
pub enum MassLevel {
    Low,
    #[default]
    Medium,
    High,
}

/// Why the engine could not run.
#[derive(Debug, thiserror::Error)]
pub enum EnergyError {
    #[error("cannot read weather file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("EPW weather file, line {line}: {message}")]
    Epw { line: usize, message: String },
    #[error(
        "unknown province {0:?}: expected one of AB, BC, MB, NB, NL, NS, NT, NU, ON, PE, QC, SK, YT"
    )]
    UnknownProvince(String),
    #[error("no window groups: at least one is needed")]
    NoWindowGroups,
    #[error("{name} must be a finite number, not {value}")]
    NotFinite { name: &'static str, value: f64 },
    #[error("{name} must be greater than zero, not {value}")]
    NotPositive { name: &'static str, value: f64 },
    #[error(
        "the solar utilisation factor for month {month} cannot be computed: a denominator is zero"
    )]
    Utilisation { month: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_default_to_murbs_run_defaults() {
        let inputs = Inputs::default();
        assert_eq!(inputs.u_walls_bg, 0.379);
        assert_eq!(inputs.f_factor, 0.93);
        assert_eq!(inputs.test_leakage, 2.0);
        assert_eq!(inputs.lpd, 7.3);
        assert_eq!(inputs.occupant_density, 25.0);
        assert_eq!(inputs.mass_level, MassLevel::Medium);
        assert_eq!(inputs.setpoint_clg, 24.0);
        assert_eq!(inputs.province, "ON");
    }
}
