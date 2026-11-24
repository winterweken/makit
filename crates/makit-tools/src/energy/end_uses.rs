//! Energy used for lighting, hot water, plug loads, and by the heating and
//! cooling plant (`models.py`).

use super::constants::FRACTION_SERVICE_WATER_HEATING_LOAD;
use super::{Inputs, Monthly};

/// Electricity for lighting over `hours` each month, kWh.
pub fn lighting_energy(inputs: &Inputs, hours: &Monthly) -> Monthly {
    let load = (inputs.lpd / 1000.0) * inputs.gfa;
    std::array::from_fn(|m| load * hours[m] * inputs.fraction_lighting)
}

/// Energy for domestic hot water over `hours` each month, kWh, through a
/// plant of seasonal COP `cop_dhw`.
pub fn domestic_hot_water(inputs: &Inputs, hours: &Monthly) -> Monthly {
    let load =
        (inputs.service_water_heating_load / 1000.0) * (inputs.gfa / inputs.occupant_density);
    std::array::from_fn(|m| {
        (load * hours[m] * FRACTION_SERVICE_WATER_HEATING_LOAD) / inputs.cop_dhw
    })
}

/// Electricity for plug loads over `hours` each month, kWh.
pub fn plug_loads(inputs: &Inputs, hours: &Monthly) -> Monthly {
    let load = (inputs.peak_receptacle_load / 1000.0) * inputs.gfa;
    std::array::from_fn(|m| load * hours[m] * inputs.fraction_receptacle_load)
}

/// Energy the heating plant uses to meet `demand`, kWh.
pub fn space_heating_energy_use(demand: &Monthly, cop: f64) -> Monthly {
    std::array::from_fn(|m| demand[m] / cop)
}

/// Energy the cooling plant uses to meet `demand`, kWh.
pub fn cooling_energy_use(demand: &Monthly, cop: f64) -> Monthly {
    std::array::from_fn(|m| demand[m] / cop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::test_support::{assert_monthly, monthly, parity, synthetic_epw};
    use crate::energy::weather::{degree_hours, Weather};
    use crate::energy::Inputs;

    #[test]
    fn the_end_uses_match_the_python_port() {
        let f = &parity()["functions"];
        let inputs: Inputs = serde_json::from_value(f["inputs"].clone()).unwrap();
        let hours = degree_hours(&Weather::read(&synthetic_epw()).unwrap(), 21.0, 24.0).hours;
        assert_monthly(
            "lighting",
            &lighting_energy(&inputs, &hours),
            &f["lighting"],
        );
        assert_monthly("dhw", &domestic_hot_water(&inputs, &hours), &f["dhw"]);
        assert_monthly("plug_loads", &plug_loads(&inputs, &hours), &f["plug_loads"]);
        assert_monthly(
            "space_heating",
            &space_heating_energy_use(&monthly(&f["transmission_htg"]), inputs.cop_htg),
            &f["space_heating"],
        );
        assert_monthly(
            "cooling",
            &cooling_energy_use(&monthly(&f["transmission_clg"]), inputs.cop_clg),
            &f["cooling"],
        );
    }
}
