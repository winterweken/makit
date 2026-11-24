//! Constants, unit conversions and standard assumptions (`static.py`).

use super::EnergyError;

// Volumetric heat capacity of air
pub const CP_AIR: f64 = 0.33; // Wh/m3K

// Energy density of natural gas
pub const ED_GAS: f64 = 10.28; // kWh/m3

// ASHRAE Handbook of Fundamentals 2017 ch.26
// Table 10 - Surface Film Coefficients/Resistances
pub const H_I_ROOF: f64 = 9.26; // W/m2K
pub const H_I_WALL: f64 = 8.29; // W/m2K
pub const H_O: f64 = 34.0; // W/m2K

// L/s >> m3/h
pub const L_PER_S_TO_M3_PER_H: f64 = 3.6;

// NECB 2017 DATA
// Table A-8.4.3.2.(2)-A
// Modeling Guidance for Loads, Operating Schedules and Illuminance Levels by Building Type
pub const OCCUPANT_DENSITY: f64 = 25.0; // m2/occupant
pub const PEAK_RECEPTACLE_LOAD: f64 = 5.0; // W/m2
pub const SERVICE_WATER_HEATING_LOAD: f64 = 500.0; // W/occupant

// Table 4.2.1.5
// Lighting Power Density by Building Type
pub const LPD: f64 = 7.3; // W/m2

// Table A-8.4.3.2.(1)-G
// Operating Schedule G
// Weekly-averaged by Jonathan Graham
pub const FRACTION_OCCUPIED: f64 = 0.677;
pub const FRACTION_LIGHTING: f64 = 0.233;
pub const FRACTION_RECEPTACLE_LOAD: f64 = 0.442;
pub const COOLING_SETPOINT: f64 = 24.0; // C
pub const HEATING_SETPOINT: f64 = 21.0; // C
pub const FRACTION_SERVICE_WATER_HEATING_LOAD: f64 = 0.308;

// ASHRAE 62.1-2019 DATA
// Table 6-1
// Minimum Ventilation Rates in Breathing Zone
pub const PEOPLE_OUTDOOR_AIR_RATE: f64 = 2.5; // L/s person
pub const AREA_OUTDOOR_AIR_RATE: f64 = 0.3; // L/s m2

// ASHRAE Handbook of Fundamentals 2017
// Chapter 9 Thermal Comfort
// Table 4 - Typical Metabolic Heat Generation for Various Activities
pub const PPL_MET_HEAT: f64 = 1.8 * 60.0; // Resting - Seated, quiet

/// The provinces and territories murb has emission factors for.
pub const PROVINCES: [&str; 13] = [
    "AB", "BC", "MB", "NB", "NL", "NS", "NT", "NU", "ON", "PE", "QC", "SK", "YT",
];

/// The emission factors for electricity and natural gas, kgCO2e/kWh, in a
/// province (two letters, any case, spaces ignored).
///
/// Canada's National Inventory Report 2021 edition: electricity from Part 3,
/// Annex 13 (2018 data); natural gas from Part 2, Annex 6, Table A6.1-1, per
/// m3 and divided by its energy density.
pub fn emission_factors(province: &str) -> Result<(f64, f64), EnergyError> {
    let gas = |kg_per_thousand_m3: f64| kg_per_thousand_m3 / 1000.0 / ED_GAS;
    let code = province.trim().to_uppercase();
    Ok(match code.as_str() {
        "QC" => (1.6 / 1000.0, gas(1887.0)),
        "MB" => (1.4 / 1000.0, gas(1886.0)),
        "BC" => (13.1 / 1000.0, gas(1926.0)),
        // PEI imports much of its electricity from New Brunswick, so New
        // Brunswick's factor is used. Its gas is not in the NIR: the mode of
        // the provinces.
        "PE" => (290.0 / 1000.0, gas(1901.0)),
        "NL" => (27.0 / 1000.0, gas(1901.0)),
        "ON" => (30.0 / 1000.0, gas(1888.0)),
        "YT" => (79.0 / 1000.0, gas(1901.0)),
        "NB" => (290.0 / 1000.0, gas(1901.0)),
        "NT" => (160.0 / 1000.0, gas(1901.0)),
        "NS" => (740.0 / 1000.0, gas(1901.0)),
        "SK" => (750.0 / 1000.0, gas(1829.0)),
        "NU" => (890.0 / 1000.0, gas(1901.0)),
        "AB" => (690.0 / 1000.0, gas(1928.0)),
        _ => return Err(EnergyError::UnknownProvince(province.to_string())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::EnergyError;

    #[test]
    fn emission_factors_accept_any_case_and_spacing() {
        let ontario = (30.0 / 1000.0, 1888.0 / 1000.0 / ED_GAS);
        assert_eq!(emission_factors(" on ").unwrap(), ontario);
        assert_eq!(emission_factors("On").unwrap(), ontario);
        assert_eq!(emission_factors("ON").unwrap(), ontario);
    }

    #[test]
    fn an_unknown_province_lists_the_valid_codes() {
        let err = emission_factors("XX").unwrap_err();
        assert!(
            matches!(&err, EnergyError::UnknownProvince(p) if p == "XX"),
            "{err:?}"
        );
        assert!(err.to_string().contains("AB, BC, MB"), "{err}");
        assert_eq!(PROVINCES.len(), 13);
        for code in PROVINCES {
            assert!(emission_factors(code).is_ok(), "{code}");
        }
    }
}
