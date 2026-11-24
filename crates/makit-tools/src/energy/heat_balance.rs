//! Heat lost and gained through the envelope, by ventilation and
//! infiltration, and from people, lights and plug loads (`heatbalance.py`).

use super::constants::{CP_AIR, H_I_ROOF, H_I_WALL, H_O, L_PER_S_TO_M3_PER_H, PPL_MET_HEAT};
use super::end_uses::{lighting_energy, plug_loads};
use super::{Inputs, Monthly};

/// Heat lost or gained through the envelope, kWh, for degree hours (kKh)
/// against the air and against the ground: above-grade walls, windows and
/// roof with their surface films, below-grade walls through the soil, and
/// slabs on grade, exposed edges and all.
pub fn transmission(
    inputs: &Inputs,
    degree_hours: &Monthly,
    degree_hours_ground: &Monthly,
) -> Monthly {
    let i = inputs;
    let u_air_walls_ag = (i.u_walls_ag.recip() + H_I_WALL.recip() + H_O.recip()).recip();
    let u_air_windows = (i.u_windows.recip() + H_I_WALL.recip() + H_O.recip()).recip();
    let u_air_roof = (i.u_roof.recip() + H_I_ROOF.recip() + H_O.recip()).recip();
    let area_envelope_ag = i.area_walls_ag + i.area_windows + i.area_roof;
    let u_avg_ag = (u_air_walls_ag * i.area_walls_ag
        + u_air_windows * i.area_windows
        + u_air_roof * i.area_roof)
        / area_envelope_ag;

    let k_soil = 1.4; // Soil conductivity, W/m2K
    let z_2 = 2.4; // Bottom depth of wall segment under consideration, m
    let z_1 = 0.3; // Top depth of wall segment under consideration, m
    let r_other = i.u_walls_bg.recip() + H_I_WALL.recip();
    let pi = std::f64::consts::PI;
    let u_avg_bw = 2.0 * k_soil / (pi * (z_2 - z_1))
        * ((z_2 + 2.0 * k_soil * r_other / pi).ln() - (z_1 + 2.0 * k_soil * r_other / pi).ln());

    let ag = u_avg_ag * area_envelope_ag;
    let bg = u_avg_bw * i.area_walls_bg;
    let sog_exposed = i.f_factor * i.perim_exp;
    let sog = 177.0_f64.recip() * i.area_sog;
    std::array::from_fn(|m| {
        ag * degree_hours[m]
            + bg * degree_hours_ground[m]
            + sog_exposed * degree_hours[m]
            + sog * degree_hours[m]
    })
}

/// Heat lost or gained to the outdoor air the ventilation brings in, kWh,
/// at a constant minimum rate (`oa_rate`, m3/h) through a heat recovery
/// ventilator of effectiveness `heat_recovery`.
pub fn ventilation(oa_rate: f64, heat_recovery: f64, degree_hours: &Monthly) -> Monthly {
    let rate = oa_rate * (1.0 - heat_recovery);
    std::array::from_fn(|m| rate * degree_hours[m] * CP_AIR)
}

/// Heat lost or gained to air leaking through the envelope, kWh: the leakage
/// at 75 Pa (L/s per m2 of above-grade envelope) brought down to a
/// year-round 5 Pa with a flow exponent of 0.60 (CaGBC Zero Carbon Building,
/// Design v2, Energy Modelling Guidelines).
pub fn infiltration(test_leakage: f64, envelope_area_ag: f64, degree_hours: &Monthly) -> Monthly {
    let infiltration_service = test_leakage * (5.0_f64 / 75.0).powf(0.60);
    let rate = infiltration_service * envelope_area_ag * L_PER_S_TO_M3_PER_H * CP_AIR;
    std::array::from_fn(|m| rate * degree_hours[m])
}

/// The minimum outdoor air rate, m3/h, as ASHRAE 62.1-2019 sets it, with a
/// zone air distribution effectiveness of 0.8 (ceiling supply and return).
pub fn outdoor_air_rate(inputs: &Inputs) -> f64 {
    let za_dist = 0.8;
    let occupants = inputs.gfa / inputs.occupant_density;
    ((inputs.people_outdoor_air_rate * occupants + inputs.area_outdoor_air_rate * inputs.gfa)
        * L_PER_S_TO_M3_PER_H)
        / za_dist
}

/// Heat from people (108 W of sensible heat each), lights and plug loads over
/// `hours` each month, kWh.
pub fn internal_gains(inputs: &Inputs, hours: &Monthly) -> Monthly {
    let people = PPL_MET_HEAT * (inputs.gfa / inputs.occupant_density);
    let lighting = lighting_energy(inputs, hours);
    let plugs = plug_loads(inputs, hours);
    std::array::from_fn(|m| {
        people * hours[m] * inputs.fraction_occupied / 1000.0 + lighting[m] + plugs[m]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::test_support::{assert_close, assert_monthly, parity, synthetic_epw};
    use crate::energy::weather::{degree_hours, Weather};
    use crate::energy::Inputs;

    #[test]
    fn the_heat_balance_matches_the_python_port() {
        let f = &parity()["functions"];
        let inputs: Inputs = serde_json::from_value(f["inputs"].clone()).unwrap();
        assert_eq!(inputs.area_sog, 950.0);
        let dh = degree_hours(&Weather::read(&synthetic_epw()).unwrap(), 21.0, 24.0);
        assert_monthly(
            "transmission_htg",
            &transmission(&inputs, &dh.htg_degree_hrs, &dh.htg_degree_hrs_ground),
            &f["transmission_htg"],
        );
        assert_monthly(
            "transmission_clg",
            &transmission(&inputs, &dh.clg_degree_hrs, &dh.clg_degree_hrs_ground),
            &f["transmission_clg"],
        );
        let oa = outdoor_air_rate(&inputs);
        assert_close(
            "outdoor_air_rate",
            oa,
            f["outdoor_air_rate"].as_f64().unwrap(),
        );
        assert_monthly(
            "ventilation_htg",
            &ventilation(oa, inputs.hrv_efficiency, &dh.htg_degree_hrs),
            &f["ventilation_htg"],
        );
        let envelope_ag = inputs.area_walls_ag + inputs.area_windows + inputs.area_roof;
        assert_monthly(
            "infiltration_htg",
            &infiltration(inputs.test_leakage, envelope_ag, &dh.htg_degree_hrs),
            &f["infiltration_htg"],
        );
        assert_monthly(
            "internal_gains_htg",
            &internal_gains(&inputs, &dh.htg_hours),
            &f["internal_gains_htg"],
        );
        assert_monthly(
            "internal_gains_clg",
            &internal_gains(&inputs, &dh.clg_hours),
            &f["internal_gains_clg"],
        );
    }
}
