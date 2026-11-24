"""Write parity.json: what murb's Python engine computes, for the Rust port's tests.

Run once, from the makit repository root, with murb's pure-Python port on
PYTHONPATH (github.com/winterweken/murb at commit a1d9e81):

    PYTHONPATH=../murb-pp python3 crates/makit-tools/src/energy/testdata/generate_parity.py

Every number comes from murb itself, on synthetic.epw beside this script. The
Rust engine in crates/makit-tools/src/energy must match them to 1e-9.
Regenerate only against the same murb commit: a different one would move the
target the port was checked against.
"""
import inspect
import json
import subprocess
from pathlib import Path

import murb_energy_tool
from murb_energy_tool import (heatbalance, models, simulation, solargains,
                              solarpos, static, utilities)

COMMIT = 'a1d9e81'
HERE = Path(__file__).resolve().parent
EPW = HERE / 'synthetic.epw'

# murb's golden scenarios, copied from murb's tests/test_simulation_parity.py.
BASE = dict(name='golden', province='AB', gfa=12000.0, area_walls_ag=4200.0,
            area_walls_bg=0.0, area_windows=1800.0, area_roof=1100.0,
            silent=True)
WG4 = [dict(pct_window_area=0.30, window_azimuth=0, shgc=0.40, shading=0.0),
       dict(pct_window_area=0.20, window_azimuth=90, shgc=0.40, shading=0.10),
       dict(pct_window_area=0.35, window_azimuth=180, shgc=0.30, shading=0.20),
       dict(pct_window_area=0.15, window_azimuth=270, shgc=0.40, shading=0.10)]
WG1 = [dict(pct_window_area=1.0, window_azimuth=180, shgc=0.40, shading=0.0)]
SCENARIOS = {
    'baseline_gas':    {**BASE, 'wg': WG1},
    'four_orient':     {**BASE, 'wg': WG4, 'area_windows': 2600.0},
    'all_electric_hp': {**BASE, 'wg': WG1, 'cop_htg': 3.0, 'cop_dhw': 2.5, 'cop_clg': 4.0},
    'low_mass':        {**BASE, 'wg': WG1, 'mass_level': 'low'},
    'half_cooled':     {**BASE, 'wg': WG1, 'clg_pct': 0.5},
    'below_grade':     {**BASE, 'wg': WG1, 'area_walls_bg': 900.0, 'area_sog': 950.0,
                        'perim_exp': 120.0, 'u_walls_bg': 0.35, 'f_factor': 0.9},
    # makit's own defaults: Ontario, a quarter of the glass each way.
    'makit_defaults':  {**BASE, 'province': 'ON',
                        'wg': [dict(pct_window_area=0.25, window_azimuth=az, shgc=0.4, shading=0.0)
                               for az in (0, 90, 180, 270)]},
}

# The 21 series murb's goldens record.
OUTPUTS = ['heating_demand', 'cooling_demand', 'heating_consumption',
           'cooling_consumption', 'lighting_consumption', 'dhw_htg_consumption',
           'plug_loads_consumption', 'total_energy_consumption',
           'electricity_consumption', 'gas_consumption', 'total_ghg_emissions',
           'transmission_heat_losses', 'transmission_heat_gains',
           'ventilation_heat_losses', 'ventilation_heat_gains',
           'infiltration_heat_losses', 'infiltration_heat_gains',
           'internal_gains_during_htg', 'internal_gains_during_clg',
           'solar_gains_during_htg', 'solar_gains_during_clg']

# Run's keyword defaults, as simulation.Run sets them.
KWARG_DEFAULTS = {
    'clg_pct': 1,
    'occupant_density': static.necb['occupant_density'],
    'peak_receptacle_load': static.necb['peak_receptacle_load'],
    'lpd': static.necb['lpd'],
    'fraction_occupied': static.necb['fraction_occupied'],
    'fraction_lighting': static.necb['fraction_lighting'],
    'fraction_receptacle_load': static.necb['fraction_receptacle_load'],
    'service_water_heating_load': static.necb['service_water_heating_load'],
    'people_outdoor_air_rate': static.ashrae_62['people_outdoor_air_rate'],
    'area_outdoor_air_rate': static.ashrae_62['area_outdoor_air_rate'],
    'mass_level': 'medium',
}

SOLAR_CASES = (
    [dict(place='Calgary', tz=-7.0, lat=51.11, lon=-114.02, month=m, day=d, hour=h)
     for m, d, h in [(1, 1, 0), (1, 1, 12), (3, 20, 6), (6, 21, 12), (6, 21, 20),
                     (9, 22, 18), (12, 21, 23)]]
    + [dict(place='Sydney', tz=10.0, lat=-33.87, lon=151.21, month=m, day=d, hour=h)
       for m, d, h in [(1, 15, 6), (7, 15, 12)]]
    + [dict(place='Quito', tz=-5.0, lat=-0.18, lon=-78.47, month=3, day=21, hour=12)])


def check_commit():
    root = Path(murb_energy_tool.__file__).resolve().parents[1]
    head = subprocess.run(['git', '-C', str(root), 'rev-parse', '--short=7', 'HEAD'],
                          capture_output=True, text=True, check=True).stdout.strip()
    if head != COMMIT:
        raise SystemExit('murb at {} is {}, not {}'.format(root, head, COMMIT))


def run_defaults():
    """Every Run argument murb gives a default, plus its keyword defaults."""
    skip = {'self', 'name', 'province', 'gfa', 'area_walls_ag', 'area_walls_bg',
            'area_windows', 'area_roof', 'window_groups', 'isd_file', 'epw_path',
            'silent', 'kwargs'}
    params = inspect.signature(simulation.Run.__init__).parameters
    named = {k: p.default for k, p in params.items()
             if k not in skip and p.default is not inspect.Parameter.empty}
    return {**named, **KWARG_DEFAULTS}


def scenario_inputs(sc):
    return {k: v for k, v in sc.items() if k not in ('name', 'silent', 'wg')}


def main():
    check_commit()
    epw, meta = utilities.process_weather_data(silent=True, epw_path=EPW)
    dh = utilities.get_degree_hours_from_weather_data(epw, 21, 24)
    out = {'source': {'repository': 'github.com/winterweken/murb', 'commit': COMMIT}}

    out['solar_position'] = []
    for c in SOLAR_CASES:
        elev, az = solarpos.solar_position(2021, c['month'], c['day'], c['hour'],
                                           c['tz'], c['lat'], c['lon'])
        out['solar_position'].append({**c, 'year': 2021, 'elevation': elev, 'azimuth': az})

    dh_fields = ['hours', 'htg_hours', 'clg_hours', 'htg_degree_hrs', 'clg_degree_hrs',
                 'htg_degree_hrs_w_clg_setpoint', 'htg_degree_hrs_ground',
                 'clg_degree_hrs_ground', 'htg_degree_hrs_w_clg_setpoint_ground']
    out['weather'] = {
        'city': meta['city'], 'latitude': meta['latitude'], 'longitude': meta['longitude'],
        'time_zone': meta['TZ'], 'elevation': meta['altitude'], 'rows': len(epw.month),
        'samples': [dict(index=i, month=epw.month[i], day=epw.day[i], hour=epw.hour[i],
                         temp_air=epw.temp_air[i], ghi=epw.ghi[i], dni=epw.dni[i],
                         dhi=epw.dhi[i], sun_elevation=epw.sun_elevation[i],
                         sun_azimuth=epw.sun_azimuth[i])
                    for i in (0, 12, 4000, 4380, 8759)],
        'degree_hours': {f: list(getattr(dh, f)) for f in dh_fields},
    }

    # Each function on its own, with the arguments Run would pass, for the
    # below_grade scenario so the below-grade and slab terms are not zero.
    i = {**run_defaults(), **scenario_inputs(SCENARIOS['below_grade'])}
    def trans(dd, dd_ground):
        return heatbalance.transmission(
            i['u_walls_ag'], i['u_walls_bg'], i['u_windows'], i['u_roof'], i['f_factor'],
            i['area_walls_ag'], i['area_walls_bg'], i['area_sog'], i['perim_exp'],
            i['area_windows'], i['area_roof'], dd, dd_ground)
    def gains(hours):
        return heatbalance.internal_gains(
            gfa=i['gfa'], hours=hours, occupancy=i['occupant_density'],
            fraction_occupied=i['fraction_occupied'], lpd=i['lpd'],
            fraction_lighting=i['fraction_lighting'],
            peak_receptacle_load=i['peak_receptacle_load'],
            fraction_receptacle_load=i['fraction_receptacle_load'])
    t_htg = trans(dh.htg_degree_hrs, dh.htg_degree_hrs_ground)
    t_clg = trans(dh.clg_degree_hrs, dh.clg_degree_hrs_ground)
    oa = heatbalance.outdoor_air_rate(i['gfa'], i['people_outdoor_air_rate'],
                                      i['area_outdoor_air_rate'], i['occupant_density'])
    v_htg = heatbalance.ventilation(oa, i['hrv_efficiency'], dh.htg_degree_hrs)
    envelope_ag = i['area_walls_ag'] + i['area_windows'] + i['area_roof']
    inf_htg = heatbalance.infiltration(i['test_leakage'], envelope_ag, dh.htg_degree_hrs)
    ig_htg, ig_clg = gains(dh.htg_hours), gains(dh.clg_hours)
    groups = [simulation.WindowGroup(**g) for g in WG4]
    sg = solargains.get_solar_gains(groups, 2600.0, epw)
    losses = t_htg + v_htg + inf_htg
    out['functions'] = {
        'inputs': i, 'window_groups': WG4, 'area_windows': 2600.0,
        'transmission_htg': list(t_htg), 'transmission_clg': list(t_clg),
        'outdoor_air_rate': oa, 'ventilation_htg': list(v_htg),
        'infiltration_htg': list(inf_htg),
        'internal_gains_htg': list(ig_htg), 'internal_gains_clg': list(ig_clg),
        'lighting': list(models.lighting_energy(i['gfa'], dh.hours, i['lpd'], i['fraction_lighting'])),
        'dhw': list(models.domestic_hot_water(i['gfa'], dh.hours, i['occupant_density'],
                                              i['cop_dhw'], i['service_water_heating_load'])),
        'plug_loads': list(models.plug_loads(i['gfa'], dh.hours, i['peak_receptacle_load'],
                                             i['fraction_receptacle_load'])),
        'space_heating': list(models.space_heating_energy_use(t_htg, i['cop_htg'])),
        'cooling': list(models.cooling_energy_use(t_clg, i['cop_clg'])),
        'solar_gains': list(sg),
        'utilisation': {m: list(solargains.utilisation_factors(sg, losses, ig_htg + ig_clg, m))
                        for m in ('low', 'medium', 'high')},
    }

    out['scenarios'] = {}
    for label, sc in SCENARIOS.items():
        inputs = scenario_inputs(sc)
        run = simulation.Run(name='parity', epw_path=EPW, silent=True,
                             window_groups=[simulation.WindowGroup(**g) for g in sc['wg']],
                             **inputs)
        out['scenarios'][label] = {
            'inputs': {**inputs, 'window_groups': sc['wg']},
            'outputs': {k: list(getattr(run, k)) for k in OUTPUTS},
            'city': run.weather_file,
        }

    with open(HERE / 'parity.json', 'w') as f:
        json.dump(out, f, indent=1, sort_keys=True)
        f.write('\n')


if __name__ == '__main__':
    main()
