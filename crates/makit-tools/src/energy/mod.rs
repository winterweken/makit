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
//!
//! The tests check every function against what murb's Python computes on a
//! synthetic weather file (`testdata/parity.json`, made by
//! `testdata/generate_parity.py`). This module depends on nothing else in
//! makit.

use std::path::PathBuf;

pub mod solar;
pub mod weather;

#[cfg(test)]
pub(crate) mod test_support;

/// A value for each month, January first.
pub type Monthly = [f64; 12];

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
