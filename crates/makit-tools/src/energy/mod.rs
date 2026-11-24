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
//! | `mod.rs` | `simulation.py` (`Run`) |
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
    #[error(
        "{name} for month {month} is not a finite number: an input is too large to compute with"
    )]
    NotFiniteOutput { name: &'static str, month: usize },
}

/// What a run gives: kWh per month (kgCO2e for emissions), named as murb's
/// `Run` names them.
#[derive(Debug, Clone, PartialEq)]
pub struct Outputs {
    pub heating_demand: Monthly,
    pub cooling_demand: Monthly,
    pub heating_consumption: Monthly,
    pub cooling_consumption: Monthly,
    pub lighting_consumption: Monthly,
    pub dhw_htg_consumption: Monthly,
    pub plug_loads_consumption: Monthly,
    pub total_energy_consumption: Monthly,
    pub electricity_consumption: Monthly,
    pub gas_consumption: Monthly,
    pub total_ghg_emissions: Monthly,
    pub transmission_heat_losses: Monthly,
    pub transmission_heat_gains: Monthly,
    pub ventilation_heat_losses: Monthly,
    pub ventilation_heat_gains: Monthly,
    pub infiltration_heat_losses: Monthly,
    pub infiltration_heat_gains: Monthly,
    pub internal_gains_during_htg: Monthly,
    pub internal_gains_during_clg: Monthly,
    pub solar_gains_during_htg: Monthly,
    pub solar_gains_during_clg: Monthly,
    /// The weather station, as its EPW names it.
    pub city: String,
}

impl Outputs {
    /// Every series with its name.
    pub fn series(&self) -> [(&'static str, &Monthly); 21] {
        [
            ("heating_demand", &self.heating_demand),
            ("cooling_demand", &self.cooling_demand),
            ("heating_consumption", &self.heating_consumption),
            ("cooling_consumption", &self.cooling_consumption),
            ("lighting_consumption", &self.lighting_consumption),
            ("dhw_htg_consumption", &self.dhw_htg_consumption),
            ("plug_loads_consumption", &self.plug_loads_consumption),
            ("total_energy_consumption", &self.total_energy_consumption),
            ("electricity_consumption", &self.electricity_consumption),
            ("gas_consumption", &self.gas_consumption),
            ("total_ghg_emissions", &self.total_ghg_emissions),
            ("transmission_heat_losses", &self.transmission_heat_losses),
            ("transmission_heat_gains", &self.transmission_heat_gains),
            ("ventilation_heat_losses", &self.ventilation_heat_losses),
            ("ventilation_heat_gains", &self.ventilation_heat_gains),
            ("infiltration_heat_losses", &self.infiltration_heat_losses),
            ("infiltration_heat_gains", &self.infiltration_heat_gains),
            ("internal_gains_during_htg", &self.internal_gains_during_htg),
            ("internal_gains_during_clg", &self.internal_gains_during_clg),
            ("solar_gains_during_htg", &self.solar_gains_during_htg),
            ("solar_gains_during_clg", &self.solar_gains_during_clg),
        ]
    }
}

/// Refuses what murb would refuse (it raises on a zero U-value, COP,
/// occupant density or above-grade envelope), and anything that is not a
/// finite number, so a run never yields NaN or infinity.
fn check(inputs: &Inputs) -> Result<(f64, f64), EnergyError> {
    let i = inputs;
    let numbers = [
        ("gfa", i.gfa),
        ("area_walls_ag", i.area_walls_ag),
        ("area_walls_bg", i.area_walls_bg),
        ("area_windows", i.area_windows),
        ("area_roof", i.area_roof),
        ("area_sog", i.area_sog),
        ("perim_exp", i.perim_exp),
        ("setpoint_htg", i.setpoint_htg),
        ("setpoint_clg", i.setpoint_clg),
        ("cop_htg", i.cop_htg),
        ("cop_clg", i.cop_clg),
        ("cop_dhw", i.cop_dhw),
        ("hrv_efficiency", i.hrv_efficiency),
        ("u_walls_ag", i.u_walls_ag),
        ("u_walls_bg", i.u_walls_bg),
        ("f_factor", i.f_factor),
        ("u_windows", i.u_windows),
        ("u_roof", i.u_roof),
        ("test_leakage", i.test_leakage),
        ("clg_pct", i.clg_pct),
        ("occupant_density", i.occupant_density),
        ("peak_receptacle_load", i.peak_receptacle_load),
        ("lpd", i.lpd),
        ("fraction_occupied", i.fraction_occupied),
        ("fraction_lighting", i.fraction_lighting),
        ("fraction_receptacle_load", i.fraction_receptacle_load),
        ("service_water_heating_load", i.service_water_heating_load),
        ("people_outdoor_air_rate", i.people_outdoor_air_rate),
        ("area_outdoor_air_rate", i.area_outdoor_air_rate),
    ];
    let groups = i.window_groups.iter().flat_map(|g| {
        [
            ("window group pct_window_area", g.pct_window_area),
            ("window group window_azimuth", g.window_azimuth),
            ("window group shgc", g.shgc),
            ("window group shading", g.shading),
        ]
    });
    for (name, value) in numbers.into_iter().chain(groups) {
        if !value.is_finite() {
            return Err(EnergyError::NotFinite { name, value });
        }
    }
    if i.window_groups.is_empty() {
        return Err(EnergyError::NoWindowGroups);
    }
    let factors = constants::emission_factors(&i.province)?;
    let positive = [
        ("gfa", i.gfa),
        ("occupant_density", i.occupant_density),
        ("u_walls_ag", i.u_walls_ag),
        ("u_walls_bg", i.u_walls_bg),
        ("u_windows", i.u_windows),
        ("u_roof", i.u_roof),
        ("cop_htg", i.cop_htg),
        ("cop_clg", i.cop_clg),
        ("cop_dhw", i.cop_dhw),
        (
            "above-grade envelope area",
            i.area_walls_ag + i.area_windows + i.area_roof,
        ),
    ];
    for (name, value) in positive {
        if value <= 0.0 {
            return Err(EnergyError::NotPositive { name, value });
        }
    }
    Ok(factors)
}

fn add(a: &Monthly, b: &Monthly) -> Monthly {
    std::array::from_fn(|m| a[m] + b[m])
}

fn zero_below_zero(v: &Monthly) -> Monthly {
    std::array::from_fn(|m| if v[m] < 0.0 { 0.0 } else { v[m] })
}

fn zero_to_one(v: &Monthly) -> Monthly {
    std::array::from_fn(|m| v[m].clamp(0.0, 1.0))
}

/// A year of the building in `weather`: murb's `Run`, step for step.
pub fn simulate(inputs: &Inputs, weather: &weather::Weather) -> Result<Outputs, EnergyError> {
    let (ghg_electricity, ghg_natural_gas) = check(inputs)?;
    let i = inputs;
    let dh = weather::degree_hours(weather, i.setpoint_htg, i.setpoint_clg);

    // The monthly solar gains through the windows
    let solar_gains_tot = solar::solar_gains(&i.window_groups, i.area_windows, weather);

    // Lighting, service water heating and plug loads
    let lighting_consumption = end_uses::lighting_energy(i, &dh.hours);
    let dhw_htg_consumption = end_uses::domestic_hot_water(i, &dh.hours);
    let plug_loads_consumption = end_uses::plug_loads(i, &dh.hours);

    // The monthly heat losses and gains
    let oa = heat_balance::outdoor_air_rate(i);
    let transmission_heat_losses =
        heat_balance::transmission(i, &dh.htg_degree_hrs, &dh.htg_degree_hrs_ground);
    let transmission_heat_gains =
        heat_balance::transmission(i, &dh.clg_degree_hrs, &dh.clg_degree_hrs_ground);
    let ventilation_heat_losses =
        heat_balance::ventilation(oa, i.hrv_efficiency, &dh.htg_degree_hrs);
    let ventilation_heat_gains =
        heat_balance::ventilation(oa, i.hrv_efficiency, &dh.clg_degree_hrs);
    let envelope_area_ag = i.area_walls_ag + i.area_windows + i.area_roof;
    let infiltration_heat_losses =
        heat_balance::infiltration(i.test_leakage, envelope_area_ag, &dh.htg_degree_hrs);
    let infiltration_heat_gains =
        heat_balance::infiltration(i.test_leakage, envelope_area_ag, &dh.clg_degree_hrs);
    let internal_gains_during_htg = heat_balance::internal_gains(i, &dh.htg_hours);
    let internal_gains_during_clg = heat_balance::internal_gains(i, &dh.clg_hours);

    // Heat losses at the cooling set-point (as RETScreen does)
    let transmission_heat_losses_clg = heat_balance::transmission(
        i,
        &dh.htg_degree_hrs_w_clg_setpoint,
        &dh.htg_degree_hrs_w_clg_setpoint_ground,
    );
    let ventilation_heat_losses_clg =
        heat_balance::ventilation(oa, i.hrv_efficiency, &dh.htg_degree_hrs_w_clg_setpoint);
    let infiltration_heat_losses_clg = heat_balance::infiltration(
        i.test_leakage,
        envelope_area_ag,
        &dh.htg_degree_hrs_w_clg_setpoint,
    );

    // How much of the sun's heat is useful for heating, and how much adds to cooling
    let hl = add(
        &add(&transmission_heat_losses, &ventilation_heat_losses),
        &infiltration_heat_losses,
    );
    let hl_prime = add(
        &add(&transmission_heat_losses_clg, &ventilation_heat_losses_clg),
        &infiltration_heat_losses_clg,
    );
    let ig = add(&internal_gains_during_htg, &internal_gains_during_clg);
    let f = solar::utilisation_factors(&solar_gains_tot, &hl, &ig, i.mass_level)?;
    let utilisation_factor_htg = zero_to_one(&f);
    let f_prime = solar::utilisation_factors(&solar_gains_tot, &hl_prime, &ig, i.mass_level)?;
    let f_prime = zero_to_one(&f_prime);
    let utilisation_factor_clg: Monthly = std::array::from_fn(|m| 1.0 - f_prime[m]);
    let solar_gains_during_htg: Monthly =
        std::array::from_fn(|m| utilisation_factor_htg[m] * solar_gains_tot[m]);
    let solar_gains_during_clg: Monthly =
        std::array::from_fn(|m| utilisation_factor_clg[m] * solar_gains_tot[m]);

    // Heating and cooling demand
    let heating_demand = zero_below_zero(&std::array::from_fn(|m| {
        transmission_heat_losses[m] + ventilation_heat_losses[m] + infiltration_heat_losses[m]
            - internal_gains_during_htg[m]
            - solar_gains_during_htg[m]
    }));
    // Prorated by the share of the floor that is cooled
    let cooling_demand: Monthly = std::array::from_fn(|m| {
        (transmission_heat_gains[m]
            + ventilation_heat_gains[m]
            + infiltration_heat_gains[m]
            + internal_gains_during_clg[m]
            + solar_gains_during_clg[m])
            * i.clg_pct
    });

    // The plant's energy use
    let heating_consumption = end_uses::space_heating_energy_use(&heating_demand, i.cop_htg);
    let cooling_consumption = end_uses::cooling_energy_use(&cooling_demand, i.cop_clg);
    let total_energy_consumption: Monthly = std::array::from_fn(|m| {
        heating_consumption[m]
            + cooling_consumption[m]
            + plug_loads_consumption[m]
            + lighting_consumption[m]
            + dhw_htg_consumption[m]
    });

    // Gas and electricity: a plant below a COP of 0.99 burns gas
    let mut electricity_consumption: Monthly = std::array::from_fn(|m| {
        cooling_consumption[m] + plug_loads_consumption[m] + lighting_consumption[m]
    });
    let mut gas_consumption = [0.0; 12];
    if i.cop_htg < 0.99 {
        gas_consumption = add(&gas_consumption, &heating_consumption);
    } else {
        electricity_consumption = add(&electricity_consumption, &heating_consumption);
    }
    if i.cop_dhw < 0.99 {
        gas_consumption = add(&gas_consumption, &dhw_htg_consumption);
    } else {
        electricity_consumption = add(&electricity_consumption, &dhw_htg_consumption);
    }
    let total_ghg_emissions: Monthly = std::array::from_fn(|m| {
        gas_consumption[m] * ghg_natural_gas + electricity_consumption[m] * ghg_electricity
    });

    let outputs = Outputs {
        heating_demand,
        cooling_demand,
        heating_consumption,
        cooling_consumption,
        lighting_consumption,
        dhw_htg_consumption,
        plug_loads_consumption,
        total_energy_consumption,
        electricity_consumption,
        gas_consumption,
        total_ghg_emissions,
        transmission_heat_losses,
        transmission_heat_gains,
        ventilation_heat_losses,
        ventilation_heat_gains,
        infiltration_heat_losses,
        infiltration_heat_gains,
        internal_gains_during_htg,
        internal_gains_during_clg,
        solar_gains_during_htg,
        solar_gains_during_clg,
        city: weather.location.city.clone(),
    };
    // Finite inputs can still overflow; never hand back infinity or NaN.
    for (name, series) in outputs.series() {
        if let Some(m) = series.iter().position(|v| !v.is_finite()) {
            return Err(EnergyError::NotFiniteOutput { name, month: m + 1 });
        }
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::test_support::{assert_monthly, parity, synthetic_epw};
    use crate::energy::weather::Weather;

    fn weather() -> Weather {
        Weather::read(&synthetic_epw()).unwrap()
    }

    fn scenario(label: &str) -> Inputs {
        serde_json::from_value(parity()["scenarios"][label]["inputs"].clone()).unwrap()
    }

    #[test]
    fn every_scenario_matches_the_python_port() {
        let weather = weather();
        let scenarios = parity()["scenarios"].as_object().unwrap();
        assert_eq!(scenarios.len(), 7);
        for (label, fixture) in scenarios {
            let outputs =
                simulate(&scenario(label), &weather).unwrap_or_else(|e| panic!("{label}: {e}"));
            for (name, got) in outputs.series() {
                assert_monthly(&format!("{label}.{name}"), got, &fixture["outputs"][name]);
            }
            assert_eq!(outputs.city, fixture["city"].as_str().unwrap(), "{label}");
        }
    }

    #[test]
    fn the_province_is_read_in_any_case() {
        let weather = weather();
        let upper = simulate(&scenario("makit_defaults"), &weather).unwrap();
        let lower = simulate(
            &Inputs {
                province: "on".into(),
                ..scenario("makit_defaults")
            },
            &weather,
        )
        .unwrap();
        assert_eq!(upper.total_ghg_emissions, lower.total_ghg_emissions);
    }

    #[test]
    fn unknown_provinces_and_missing_windows_are_errors() {
        let weather = weather();
        let err = simulate(
            &Inputs {
                province: "XX".into(),
                ..scenario("makit_defaults")
            },
            &weather,
        )
        .unwrap_err();
        assert!(
            matches!(&err, EnergyError::UnknownProvince(p) if p == "XX"),
            "{err:?}"
        );
        let err = simulate(
            &Inputs {
                window_groups: Vec::new(),
                ..scenario("makit_defaults")
            },
            &weather,
        )
        .unwrap_err();
        assert!(matches!(err, EnergyError::NoWindowGroups), "{err:?}");
    }

    #[test]
    fn inputs_murb_would_divide_by_zero_with_are_refused() {
        let weather = weather();
        let base = scenario("makit_defaults");
        type Zero = fn(&mut Inputs);
        let fields: [(&str, Zero); 9] = [
            ("gfa", |i| i.gfa = 0.0),
            ("occupant_density", |i| i.occupant_density = 0.0),
            ("u_walls_ag", |i| i.u_walls_ag = 0.0),
            ("u_walls_bg", |i| i.u_walls_bg = 0.0),
            ("u_windows", |i| i.u_windows = 0.0),
            ("u_roof", |i| i.u_roof = 0.0),
            ("cop_htg", |i| i.cop_htg = 0.0),
            ("cop_clg", |i| i.cop_clg = 0.0),
            ("cop_dhw", |i| i.cop_dhw = 0.0),
        ];
        for (field, zero) in fields {
            let mut inputs = base.clone();
            zero(&mut inputs);
            let err = simulate(&inputs, &weather).unwrap_err();
            assert!(
                matches!(err, EnergyError::NotPositive { name, .. } if name == field),
                "{field}: {err:?}"
            );
        }
        let err = simulate(
            &Inputs {
                area_walls_ag: 0.0,
                area_windows: 0.0,
                area_roof: 0.0,
                ..base.clone()
            },
            &weather,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                EnergyError::NotPositive {
                    name: "above-grade envelope area",
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(err
            .to_string()
            .contains("above-grade envelope area must be greater than zero"));
    }

    #[test]
    fn a_result_that_overflows_is_an_error_not_infinity() {
        let inputs = Inputs {
            area_walls_bg: 1e308,
            ..scenario("makit_defaults")
        };
        let err = simulate(&inputs, &weather()).unwrap_err();
        assert!(
            matches!(err, EnergyError::NotFiniteOutput { .. }),
            "{err:?}"
        );
    }

    #[test]
    fn non_finite_inputs_are_refused() {
        let weather = weather();
        let base = scenario("makit_defaults");
        let err = simulate(
            &Inputs {
                area_walls_bg: f64::NAN,
                ..base.clone()
            },
            &weather,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                EnergyError::NotFinite {
                    name: "area_walls_bg",
                    ..
                }
            ),
            "{err:?}"
        );
        let err = simulate(
            &Inputs {
                hrv_efficiency: f64::INFINITY,
                ..base.clone()
            },
            &weather,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                EnergyError::NotFinite {
                    name: "hrv_efficiency",
                    ..
                }
            ),
            "{err:?}"
        );
        let mut inputs = base.clone();
        inputs.window_groups[2].shgc = f64::NAN;
        let err = simulate(&inputs, &weather).unwrap_err();
        assert!(
            matches!(
                err,
                EnergyError::NotFinite {
                    name: "window group shgc",
                    ..
                }
            ),
            "{err:?}"
        );
    }

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
