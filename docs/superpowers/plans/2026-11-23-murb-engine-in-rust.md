# MURB Energy Engine in Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `murb-simulate`, the TUI's energy run and `murb-report` work from the makit binary alone, with a Rust port of murb's energy model in place of the Python bridge.

**Architecture:** A new module `crates/makit-tools/src/energy/` ports the parts of `murb_energy_tool` that makit uses, file for file, as plain functions over `[f64; 12]` monthly series. `murb.rs` builds the engine's `Inputs` from its `MurbInput`, runs it in-process and keeps every series in the results; the report page gains a heat-flow section from them. Parity with the Python port is pinned by a JSON fixture the Python generates once on a committed synthetic weather file. murb's numpy-era goldens are not checked here: the Calgary file they were made on has no confirmed licence, so the design's synthetic-weather fallback applies, and murb's own tests already tie the Python port to them.

**Tech Stack:** Rust 2021 workspace, `thiserror` 2 (workspace dependency, new to makit-tools), `serde_json` in tests only; Python 3 with `../murb-pp` on `PYTHONPATH` for the one-time fixture generation.

**Spec:** `docs/superpowers/specs/2026-11-23-murb-engine-in-rust-design.md`

## Global Constraints

- Work on a branch `murb-engine-rust` cut from `dev`; commits authored with the GitHub noreply address already in this checkout's `git config user.email`; messages short and imperative, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Never commit `murb_results.json`, `murb_results.html`, `results/` or `examples/EPW/` from the repository root; they are the user's local files.
- The Python source of truth is `/Users/inscrip/code/murb-pp/murb_energy_tool/`, a worktree at murb commit `a1d9e81` (merged into murb `main` as `83cf3b8`). Port it module for module and keep Python's left-to-right order of operations.
- Python's `%` takes the divisor's sign: every `% 360.0` and `% 1440.0` becomes `rem_euclid`. Never Rust's `%` on a value that can be negative.
- `crates/makit-tools/src/energy/` uses only `std`, `thiserror`, and its own files. It never imports `makit_core`, `crate::murb`, `crate::ifc` or any other makit-tools module. `serde` and `serde_json` appear in it only under `#[cfg(test)]`.
- Parity tolerance everywhere: `(got - want).abs() <= 1e-9 + 1e-9 * want.abs()`.
- The engine fails wherever murb raises, and on non-finite inputs; it never returns NaN or infinity.
- Results files written by the Python bridge must still load: no `deny_unknown_fields`, removed fields simply disappear, new fields are `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- Run Rust tests as `env -u MAKIT_PYTHON cargo test -q ...` (a relative `MAKIT_PYTHON` breaks an unrelated ifc test). `cargo fmt` and `cargo clippy --all-targets` clean before every commit.

## Review Focus

1. **A leap-year (AMY) weather file**, with a February 29 row: the user gets an error naming the line, not a crash or a silently shifted year. Pinned in Task 2.
2. **Weather from the southern or eastern hemisphere**: the sun's azimuth branch differs, and the result must match murb. Pinned by the Sydney case in Task 1's fixture.
3. **A province typed in lower case or with spaces** (`on`, ` ON `): it is accepted and gives Ontario's factors. Pinned in Task 4.
4. **`nan` or `inf` typed as an option**: Rust's `f64` parser accepts both. The run must fail naming the input, not write NaN into results. Pinned in Task 5.
5. **A results file written by the Python bridge**, with `report_path`, `report_error` and `input.report_dir`: `murb-report` and the TUI still load it, and the page simply has no heat-flow section. Pinned in Tasks 6 and 7.

---

### Task 1: Engine scaffold, synthetic weather, parity fixture and solar position

**Files:**
- Modify: `crates/makit-tools/Cargo.toml` (add `thiserror = { workspace = true }` to `[dependencies]`)
- Modify: `crates/makit-tools/src/lib.rs` (add `pub mod energy;`)
- Create: `crates/makit-tools/src/energy/mod.rs`
- Create: `crates/makit-tools/src/energy/solar.rs`
- Create: `crates/makit-tools/src/energy/test_support.rs`
- Create: `crates/makit-tools/src/energy/testdata/make_synthetic_epw.py`
- Create: `crates/makit-tools/src/energy/testdata/synthetic.epw` (generated)
- Create: `crates/makit-tools/src/energy/testdata/generate_parity.py`
- Create: `crates/makit-tools/src/energy/testdata/parity.json` (generated)

**Interfaces:**
- Produces:
  - `pub type Monthly = [f64; 12];` in `energy/mod.rs`.
  - `pub enum EnergyError` in `energy/mod.rs` (thiserror), with exactly these variants and messages:
    - `Io { path: PathBuf, source: std::io::Error }`: `"cannot read weather file {path}: {source}"` (path via `.display()`).
    - `Epw { line: usize, message: String }`: `"EPW weather file, line {line}: {message}"`.
    - `UnknownProvince(String)`: `"unknown province {0:?}: expected one of AB, BC, MB, NB, NL, NS, NT, NU, ON, PE, QC, SK, YT"`.
    - `NoWindowGroups`: `"no window groups: at least one is needed"`.
    - `NotFinite { name: &'static str, value: f64 }`: `"{name} must be a finite number, not {value}"`.
    - `NotPositive { name: &'static str, value: f64 }`: `"{name} must be greater than zero, not {value}"`.
    - `Utilisation { month: usize }`: `"the solar utilisation factor for month {month} cannot be computed: a denominator is zero"`.
  - `pub fn solar::solar_position(year: i32, month: u32, day: u32, hour: u32, tz: f64, lat: f64, lon: f64) -> (f64, f64)`, returning (true elevation, azimuth clockwise from north) in degrees.
  - `pub(crate) fn solar::day_ordinal(year: i32, month: u32, day: u32) -> i64`: Python's `date.toordinal()`, where 0001-01-01 is 1, proleptic Gregorian.
  - `#[cfg(test)] pub(crate) mod test_support` with `pub fn parity() -> &'static serde_json::Value` (loaded once through `OnceLock` from `testdata/parity.json` via `env!("CARGO_MANIFEST_DIR")`), `pub fn synthetic_epw() -> PathBuf`, `pub fn assert_close(label: &str, got: f64, want: f64)` and `pub fn assert_monthly(label: &str, got: &Monthly, want: &serde_json::Value)`. Both asserts use the global tolerance and name the label and month on failure.
  - `testdata/parity.json`, whose shape every later task reads:

```text
{
  "source":   {"repository": "github.com/winterweken/murb", "commit": "a1d9e81"},
  "solar_position": [ {year, month, day, hour, tz, lat, lon, elevation, azimuth}, ... ],
  "weather":  {city, latitude, longitude, time_zone, elevation, rows,
               samples: [ {index, month, day, hour, temp_air, ghi, dni, dhi, sun_elevation, sun_azimuth}, ... ],
               degree_hours: {hours, htg_hours, clg_hours, htg_degree_hrs, clg_degree_hrs,
                              htg_degree_hrs_w_clg_setpoint, htg_degree_hrs_ground,
                              clg_degree_hrs_ground, htg_degree_hrs_w_clg_setpoint_ground}},
  "functions": {inputs, window_groups, area_windows,
                transmission_htg, transmission_clg, outdoor_air_rate, ventilation_htg,
                infiltration_htg, internal_gains_htg, internal_gains_clg,
                lighting, dhw, plug_loads, space_heating, cooling,
                solar_gains, utilisation: {low, medium, high}},
  "scenarios": { label: {inputs, outputs: {21 series}, city}, ... }
}
```

- [ ] **Step 1: Write `make_synthetic_epw.py`** (stdlib only, deterministic, no randomness) that writes `synthetic.epw` beside itself with LF line endings:
  - Line 1: `LOCATION,SYNTHETIC CALGARY,AB,CAN,makit synthetic weather,000000,51.11,-114.02,-7.0,1084.1`. Lines 2 to 8: the standard EPW header records (`DESIGN CONDITIONS,0`, `TYPICAL/EXTREME PERIODS,0`, `GROUND TEMPERATURES,0`, `HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0`, two `COMMENTS` lines saying the file is synthetic and made by this script, `DATA PERIODS,1,1,Data,Friday, 1/ 1,12/31`).
  - 8,760 rows, days 1 to 365 of 2021, hours 1 to 24. With `doy` the day of the year and `h` the EPW hour:
    - `temp = 4.5 - 16.0*cos(2π(doy-18)/365) + 6.5*cos(2π(h-15)/24)`, written to one decimal.
    - Sun: `decl = 23.44*sin(2π(284+doy)/365)`, `ha = 15*((h - 0.5) + (lon - 15*tz)/15 - 12)` in degrees, `s = sin(lat)sin(decl) + cos(lat)cos(decl)cos(ha)`, `cloud = 0.35 + 0.65*((doy*37) % 11)/10`.
    - When `s > 0`: `dni = round(900*exp(-0.14/s)*cloud)`, `dhi = round(60 + 110*(1-cloud)*s + 40*s)`, `ghi = round(dni*s + dhi)`. Otherwise all three are 0.
  - Fill all 35 fields: year 2021, month, day, hour, minute 60, then EPW missing-value codes everywhere except dry bulb (field 6), GHI (13), DNI (14) and DHI (15). makit and murb read only fields 1, 2, 3, 6, 13, 14 and 15.

- [ ] **Step 2: Generate the weather file**

Run: `python3 crates/makit-tools/src/energy/testdata/make_synthetic_epw.py && wc -l crates/makit-tools/src/energy/testdata/synthetic.epw`
Expected: `8768` lines.

- [ ] **Step 3: Write `generate_parity.py`.** It imports `murb_energy_tool` from `PYTHONPATH` and refuses to run unless `git -C <murb root> rev-parse --short=7 HEAD` prints `a1d9e81`, where the murb root is two levels above `murb_energy_tool.__file__`. It writes `parity.json` beside itself with `json.dump(..., indent=1, sort_keys=True)`. Floats keep Python's `repr`, so they round-trip exactly. The sections:
  - `solar_position`: `solarpos.solar_position` for year 2021 at Calgary (tz -7, lat 51.11, lon -114.02) on (1,1,0), (1,1,12), (3,20,6), (6,21,12), (6,21,20), (9,22,18), (12,21,23); at Sydney (tz 10, lat -33.87, lon 151.21) on (1,15,6), (7,15,12); and at Quito (tz -5, lat -0.18, lon -78.47) on (3,21,12).
  - `weather`: from `utilities.process_weather_data(silent=True, epw_path=synthetic.epw)`, the location metadata and row count. Add samples at indices 0, 12, 4000, 4380 and 8759, and every field of `utilities.get_degree_hours_from_weather_data(epw, 21, 24)`.
  - `functions`: `inputs` is murb's `below_grade` golden scenario. That is `BASE` from `tests/test_simulation_parity.py` plus `area_walls_bg=900, area_sog=950, perim_exp=120, u_walls_bg=0.35, f_factor=0.9`, with every other `Run` default written out explicitly under its `Run` keyword name. Use `window_groups` = murb's `WG4` and `area_windows` = 2600. Compute each listed series from `heatbalance`, `models` and `solargains`, using the arguments `Run` would pass. `space_heating` is `space_heating_energy_use(transmission_htg, cop_htg)`. `cooling` is `cooling_energy_use(transmission_clg, cop_clg)`. `utilisation[mass]` is `utilisation_factors(solar_gains, transmission_htg + ventilation_htg + infiltration_htg, internal_gains_htg + internal_gains_clg, mass)`.
  - `scenarios`: murb's six golden scenarios, copied from `tests/test_simulation_parity.py` (`baseline_gas`, `four_orient`, `all_electric_hp`, `low_mass`, `half_cooled`, `below_grade`), plus `makit_defaults`. That one is province `ON`, the `BASE` areas, and four groups of `pct_window_area` 0.25 at azimuths 0, 90, 180 and 270, with shgc 0.4 and shading 0. Run each through `simulation.Run(name='parity', epw_path=synthetic.epw, silent=True, window_groups=..., **inputs)`. Record as `inputs` the scenario's keyword arguments without `name`, `silent` and `wg`, plus `window_groups` as a list of dicts. Record the 21 series murb's goldens record as `outputs`, and `run.weather_file` as `city`. (`functions.inputs` has no `window_groups`; its groups sit in `functions.window_groups`.)

- [ ] **Step 4: Generate the fixture**

Run: `PYTHONPATH=../murb-pp python3 crates/makit-tools/src/energy/testdata/generate_parity.py && python3 -c "import json; d=json.load(open('crates/makit-tools/src/energy/testdata/parity.json')); print(d['source']['commit'], len(d['scenarios']), d['weather']['rows'])"`
Expected: `a1d9e81 7 8760`

- [ ] **Step 5: Write the failing tests** in `energy/solar.rs`:

```rust
#[test]
fn day_ordinal_matches_pythons_toordinal() {
    assert_eq!(day_ordinal(1, 1, 1), 1);
    assert_eq!(day_ordinal(2021, 1, 1), 737791);
    assert_eq!(day_ordinal(2021, 12, 31), 738155);
    assert_eq!(day_ordinal(2000, 3, 1), 730180);
}
```

  And `solar_position_matches_the_python_port`: for every case in `parity()["solar_position"]`, `solar_position` with the case's year, month, day, hour, tz, lat and lon gives an elevation and azimuth that pass `assert_close` against the case's `elevation` and `azimuth`, labelled with the case's place and date.

- [ ] **Step 6: Run the tests to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: a compile failure naming `solar_position` and `day_ordinal`.

- [ ] **Step 7: Implement** `energy/mod.rs` and `energy/solar.rs`:
  - `mod.rs` has a module doc comment saying this is a port of `murb_energy_tool` from github.com/winterweken/murb at `a1d9e81`, and listing which murb module each file ports.
  - `mod.rs` declares `pub mod solar;` and `#[cfg(test)] pub(crate) mod test_support;`, defines `Monthly` and `EnergyError`, and contains nothing else yet.
  - `solar_position` ports `solarpos.py` line for line, using `rem_euclid` for both modulos.
  - `day_ordinal` computes days from 0001-01-01 arithmetically, with no date crate.

- [ ] **Step 8: Run the tests to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: `2 passed`.

- [ ] **Step 9: Commit**

```bash
git add crates/makit-tools/Cargo.toml crates/makit-tools/src/lib.rs crates/makit-tools/src/energy Cargo.lock
git commit -m "Start the energy engine with solar position and its parity fixture"
```

### Task 2: Weather reader and degree hours

**Files:**
- Create: `crates/makit-tools/src/energy/weather.rs`
- Modify: `crates/makit-tools/src/energy/mod.rs` (add `pub mod weather;`)

**Interfaces:**
- Consumes: `solar::solar_position`, `EnergyError`, `Monthly`, `test_support`.
- Produces:
  - `pub struct Location { pub city: String, pub latitude: f64, pub longitude: f64, pub time_zone: f64, pub elevation: f64 }`
  - `pub fn parse_location(line: &str) -> Result<Location, EnergyError>`: the `LOCATION` header with fields 1, 6, 7, 8 and 9. A BOM before `LOCATION` is accepted. Anything else is `Epw { line: 1, .. }`.
  - `pub struct Hour { pub month: u32, pub day: u32, pub hour: u32, pub temp_air: f64, pub ghi: f64, pub dni: f64, pub dhi: f64, pub sun_elevation: f64, pub sun_azimuth: f64 }`. `hour` is 0 to 23, the start of the hour, as murb stores it.
  - `pub struct Weather { pub location: Location, pub hours: Vec<Hour> }` with `pub fn parse(text: &str) -> Result<Weather, EnergyError>`, `pub fn read(path: &Path) -> Result<Weather, EnergyError>` and `pub fn monthly_sum(&self, value: impl Fn(&Hour) -> f64) -> Monthly`. `monthly_sum` adds row by row in file order, as murb does.
  - `pub struct DegreeHours` with the nine `Monthly` fields named exactly as murb's `MonthlyWeather`, and `pub fn degree_hours(weather: &Weather, setpoint_htg: f64, setpoint_clg: f64) -> DegreeHours`.

- [ ] **Step 1: Write the failing tests** in `weather.rs`:
  - `the_synthetic_weather_matches_the_python_reader`: `Weather::read(&synthetic_epw())`. Then:
    - The location equals the fixture's `weather` fields, and `hours.len() == 8760`.
    - Each sample's fields match the fixture through `assert_close`, `hour` included as stored.
    - `degree_hours(&w, 21.0, 24.0)` matches all nine fixture series through `assert_monthly`.
  - `a_byte_order_mark_and_crlf_line_endings_are_accepted`: prefix the synthetic text with `'\u{feff}'` and turn `\n` into `\r\n`. The parse gives 8,760 rows and the same first row.
  - `invalid_utf8_is_replaced_not_fatal`: write the synthetic bytes with the city's `C` replaced by byte `0xE9` to a temp file. `read` succeeds and the city contains `'\u{FFFD}'`.
  - `blank_lines_after_the_header_are_skipped`: insert an empty line after line 20. The parse still gives 8,760 rows.
  - `a_short_row_names_its_line`: replace line 9 with `2021,1,1,1,60`. The error is `Epw { line: 9, .. }`, and its message contains `"at least 16 fields"`.
  - `a_field_that_is_not_a_number_names_its_line`: put `warm` in field 6 of line 10. The error is `Epw { line: 10, .. }` and names `warm`.
  - `an_hour_outside_1_to_24_is_an_error`: an hour of 25 on line 9 gives `Epw { line: 9, .. }`.
  - `february_29_is_an_error_naming_its_line` (Review Focus 1): make line 9's date `2,29`. The error is `Epw { line: 9, .. }` with a message containing `"not a date"`.
  - `a_file_without_a_location_header_is_an_error`: drop line 1. The error is `Epw { line: 1, .. }`.
  - `a_file_with_no_hourly_rows_is_an_error`: keep only the 8 header lines. The error is an `Epw` whose message contains `"no hourly data"`.
  - `reading_a_missing_file_names_its_path`: the `read` error's message contains the path and `"cannot read weather file"`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::weather`
Expected: a compile failure naming `Weather`.

- [ ] **Step 3: Implement `weather.rs`** to port `epw.py` and the weather half of `utilities.py`:
  - Data starts at line 9. Fields are trimmed before parsing. The year is fixed at 2021, and a date that is not in 2021 (February 29, day 31 of a 30-day month, month 13) is an error.
  - `read` uses `fs::read` and `String::from_utf8_lossy`. `str::lines` handles CRLF.
  - Each row's sun position comes from `solar_position(2021, month, day, hour - 1, tz, lat, lon)`.
  - `degree_hours` keeps murb's comparisons exactly. A heating hour is `x > setpoint_htg ? 0 : 1`, and a cooling hour is `x < setpoint_clg ? 0 : 1`. The ground series add the mean-temperature term once per row, not hours times value.

- [ ] **Step 4: Run them to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: all energy tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/energy
git commit -m "Read EPW weather and its degree hours in the energy engine"
```

### Task 3: Window groups, solar gains and utilisation factors

**Files:**
- Modify: `crates/makit-tools/src/energy/mod.rs` (add `WindowGroup`, `MassLevel`)
- Modify: `crates/makit-tools/src/energy/solar.rs`

**Interfaces:**
- Consumes: `Weather`, `Hour`, `Monthly`, `EnergyError::Utilisation`.
- Produces:
  - `pub struct WindowGroup { pub pct_window_area: f64, pub window_azimuth: f64, pub shgc: f64, pub shading: f64 }`, where `Default` gives `0.0, 0.0, 0.4, 0.0`. Under `#[cfg_attr(test, derive(serde::Deserialize), serde(default))]`, so fixtures deserialize straight into it.
  - `pub enum MassLevel { Low, Medium, High }`, where `Default` is `Medium`, under `#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "lowercase"))]`. murb treats any value other than `low` or `medium` as high; the enum makes that explicit.
  - `pub fn solar::solar_gains(window_groups: &[WindowGroup], area_windows: f64, weather: &Weather) -> Monthly`
  - `pub fn solar::utilisation_factors(solar_gains: &Monthly, heat_losses: &Monthly, internal_gains: &Monthly, mass: MassLevel) -> Result<Monthly, EnergyError>`

- [ ] **Step 1: Write the failing tests** in `solar.rs`:
  - `solar_gains_match_the_python_port`: deserialize `functions.window_groups` into `Vec<WindowGroup>`. Then `solar_gains(&groups, 2600.0, &Weather::read(&synthetic_epw())?)` matches `functions.solar_gains`.
  - `utilisation_factors_match_for_every_mass_level`: for low, medium and high, the result matches `functions.utilisation[mass]`, with heat losses = `transmission_htg + ventilation_htg + infiltration_htg` and internal gains = `internal_gains_htg + internal_gains_clg`, summed month by month from the fixture.
  - `a_zero_denominator_is_an_error_not_nan`: solar gains are 100 in every month. With heat losses and internal gains both 50 in every month, the result is `Err(EnergyError::Utilisation { month: 1 })`, the first month that fails. With heat losses 80 in every month and internal gains 50 except 80 in month 3, it is `Err(Utilisation { month: 3 })`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::solar`
Expected: a compile failure naming `solar_gains`.

- [ ] **Step 3: Implement both functions** by porting `solargains.py`. The 0.93 attenuation and the `/ 1000.0` stay in murb's order. A month whose `glr` or factor is not finite returns `Utilisation { month }`, numbered 1 to 12. That catches both denominators, because a zero in either yields infinity or NaN.

- [ ] **Step 4: Run them to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: all energy tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/energy
git commit -m "Port murb's solar gains and utilisation factors"
```

### Task 4: Inputs, constants, heat balance and end uses

**Files:**
- Modify: `crates/makit-tools/src/energy/mod.rs` (add `Inputs`)
- Create: `crates/makit-tools/src/energy/constants.rs`
- Create: `crates/makit-tools/src/energy/heat_balance.rs`
- Create: `crates/makit-tools/src/energy/end_uses.rs`

**Interfaces:**
- Consumes: `Monthly`, `WindowGroup`, `MassLevel`, `EnergyError::UnknownProvince`, `weather::degree_hours`.
- Produces:
  - `pub struct Inputs` with one public field per `Run` argument or honoured keyword, named exactly as murb names them: `province: String`, `gfa`, `area_walls_ag`, `area_walls_bg`, `area_windows`, `area_roof`, `window_groups: Vec<WindowGroup>`, `area_sog`, `perim_exp`, `setpoint_htg`, `setpoint_clg`, `cop_htg`, `cop_clg`, `cop_dhw`, `hrv_efficiency`, `u_walls_ag`, `u_walls_bg`, `f_factor`, `u_windows`, `u_roof`, `test_leakage`, `clg_pct`, `occupant_density`, `peak_receptacle_load`, `lpd`, `fraction_occupied`, `fraction_lighting`, `fraction_receptacle_load`, `service_water_heating_load`, `people_outdoor_air_rate`, `area_outdoor_air_rate`, and `mass_level: MassLevel`. Every unnamed type is `f64`. `Default` gives murb's defaults: province `"ON"`, the five areas 0, no window groups, `area_sog` 0, `perim_exp` 0, setpoints 21 and 24, COPs 0.85, 5.2 and 0.85, HRV 0.55, U-values 0.273 (walls ag), 0.379 (walls bg), 2.56 (windows) and 0.164 (roof), `f_factor` 0.93, `test_leakage` 2, `clg_pct` 1, then the NECB and ASHRAE values from `static.py` (occupant density 25, receptacle 5, LPD 7.3, fractions 0.677, 0.233 and 0.442, hot water 500, outdoor air 2.5 and 0.3), and mass `Medium`. Under `#[cfg_attr(test, derive(serde::Deserialize), serde(default))]`.
  - `constants.rs`: every number in `static.py` as a named `pub const` (`CP_AIR`, `ED_GAS`, `H_I_ROOF`, `H_I_WALL`, `H_O`, `L_PER_S_TO_M3_PER_H`, `PPL_MET_HEAT`, `FRACTION_SERVICE_WATER_HEATING_LOAD`, and the NECB and ASHRAE 62.1 values used for `Inputs::default`), with `static.py`'s source comments carried over, the Jonathan Graham credit included. It also has `pub const PROVINCES: [&str; 13]` in alphabetical order, and `pub fn emission_factors(province: &str) -> Result<(f64, f64), EnergyError>`, which trims, uppercases and returns (electricity, natural gas) in kgCO2e/kWh. Natural gas is computed as `n / 1000.0 / ED_GAS`, in murb's order.
  - `heat_balance.rs`: `pub fn transmission(inputs: &Inputs, degree_hours: &Monthly, degree_hours_ground: &Monthly) -> Monthly`, `pub fn ventilation(oa_rate: f64, heat_recovery: f64, degree_hours: &Monthly) -> Monthly`, `pub fn infiltration(test_leakage: f64, envelope_area_ag: f64, degree_hours: &Monthly) -> Monthly`, `pub fn outdoor_air_rate(inputs: &Inputs) -> f64` (zone air distribution 0.8) and `pub fn internal_gains(inputs: &Inputs, hours: &Monthly) -> Monthly`.
  - `end_uses.rs`: `pub fn lighting_energy(inputs: &Inputs, hours: &Monthly) -> Monthly`, `pub fn domestic_hot_water(inputs: &Inputs, hours: &Monthly) -> Monthly`, `pub fn plug_loads(inputs: &Inputs, hours: &Monthly) -> Monthly`, `pub fn space_heating_energy_use(demand: &Monthly, cop: f64) -> Monthly` and `pub fn cooling_energy_use(demand: &Monthly, cop: f64) -> Monthly`.

- [ ] **Step 1: Write the failing tests:**
  - In `heat_balance.rs`, `the_heat_balance_matches_the_python_port`: deserialize `functions.inputs` into `Inputs` and take `degree_hours(&synthetic, 21.0, 24.0)`. Then these match the fixture: `transmission` with heating and with cooling degree hours, `outdoor_air_rate`, `ventilation` and `infiltration` with heating degree hours, and `internal_gains` with `htg_hours` and with `clg_hours`.
  - In `end_uses.rs`, `the_end_uses_match_the_python_port`: lighting, hot water and plug loads with `hours` match. So do `space_heating_energy_use(fixture transmission_htg, cop_htg)` and `cooling_energy_use(fixture transmission_clg, cop_clg)`.
  - In `constants.rs`:
    - `emission_factors_accept_any_case_and_spacing` (Review Focus 3): `" on "` and `"On"` both give `(30.0 / 1000.0, 1888.0 / 1000.0 / ED_GAS)`.
    - `an_unknown_province_lists_the_valid_codes`: `"XX"` gives `UnknownProvince`, whose message contains `"AB, BC, MB"`.
  - In `mod.rs`, `inputs_default_to_murbs_run_defaults`: assert `u_walls_bg == 0.379`, `f_factor == 0.93`, `test_leakage == 2.0`, `lpd == 7.3`, `occupant_density == 25.0`, `mass_level == MassLevel::Medium`, `setpoint_clg == 24.0` and `province == "ON"`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: a compile failure naming `Inputs`.

- [ ] **Step 3: Implement** the three files by porting `static.py`, `heatbalance.py` and `models.py`. A Python `x ** -1` becomes `1.0 / x`, and `177**-1` becomes `1.0 / 177.0`. `math.pi` and `math.log` become `std::f64::consts::PI` and `f64::ln`. Keep murb's order of multiplication. For example, lighting is `(lpd / 1000.0) * gfa * hours[m] * fraction_lighting`.

- [ ] **Step 4: Run them to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: all energy tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/energy
git commit -m "Port murb's constants, heat balance and end-use models"
```

### Task 5: The simulation run, its checks and full parity

**Files:**
- Modify: `crates/makit-tools/src/energy/mod.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 to 4.
- Produces:
  - `pub struct Outputs` with 21 `pub ... : Monthly` fields named exactly as murb's `Run` attributes. The heating and cooling series are `heating_demand`, `cooling_demand`, `heating_consumption` and `cooling_consumption`. The end uses are `lighting_consumption`, `dhw_htg_consumption`, `plug_loads_consumption` and `total_energy_consumption`. Then come `electricity_consumption`, `gas_consumption` and `total_ghg_emissions`. The heat flows are `transmission_heat_losses`, `transmission_heat_gains`, `ventilation_heat_losses`, `ventilation_heat_gains`, `infiltration_heat_losses`, `infiltration_heat_gains`, `internal_gains_during_htg`, `internal_gains_during_clg`, `solar_gains_during_htg` and `solar_gains_during_clg`. Last is `pub city: String`.
  - `pub fn simulate(inputs: &Inputs, weather: &Weather) -> Result<Outputs, EnergyError>`.

- [ ] **Step 1: Write the failing tests** in `mod.rs`:
  - `every_scenario_matches_the_python_port`: for each of the seven fixture scenarios, deserialize `inputs` into `Inputs` and run `simulate` on the synthetic weather. All 21 `outputs` series match through `assert_monthly`, with the label `"{scenario}.{series}"`, and `city` equals the fixture's.
  - `the_province_is_read_in_any_case`: `makit_defaults` with province `"on"` gives the same `total_ghg_emissions` as with `"ON"`.
  - `unknown_provinces_and_missing_windows_are_errors`: `"XX"` gives `UnknownProvince("XX")`, and an empty `window_groups` gives `NoWindowGroups`.
  - `inputs_murb_would_divide_by_zero_with_are_refused`: starting from `makit_defaults`, each of these set to 0.0 gives `NotPositive` with that exact `name`: `gfa`, `occupant_density`, `u_walls_ag`, `u_walls_bg`, `u_windows`, `u_roof`, `cop_htg`, `cop_clg` and `cop_dhw`. Setting `area_walls_ag`, `area_windows` and `area_roof` all to 0 gives `NotPositive { name: "above-grade envelope area", .. }`.
  - `non_finite_inputs_are_refused` (Review Focus 4): `area_walls_bg = f64::NAN` gives `NotFinite { name: "area_walls_bg", .. }`. `hrv_efficiency = f64::INFINITY` gives `NotFinite { name: "hrv_efficiency", .. }`. A window group with `shgc = NAN` gives `NotFinite { name: "window group shgc", .. }`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::tests`
Expected: a compile failure naming `simulate`.

- [ ] **Step 3: Implement `simulate`.** Checks come first, in this order:
  1. Every numeric field of `Inputs` and of each window group is finite. Window group fields are named `window group pct_window_area`, `window group window_azimuth`, `window group shgc` and `window group shading`.
  2. `window_groups` is not empty.
  3. The province is known, through `emission_factors`.
  4. The positive checks listed in the test above hold.

  Then port the body of `Run.__init__` from `simulation.py`, from the weather onward, in its order. That includes:
  - `clg_pct` prorating cooling demand.
  - The `cop < 0.99` gas thresholds.
  - The `set_zero_below_zero` and `set_zero_to_one` clamps.
  - The utilisation factors computed for heating and, with the cooling set-point losses, for cooling.

  `degree_hours` uses `inputs.setpoint_htg` and `inputs.setpoint_clg`.

- [ ] **Step 4: Run them to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools energy::`
Expected: all energy tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/energy
git commit -m "Run murb's monthly energy model in Rust, matching the Python port"
```

### Task 6: makit runs the engine; the Python bridge goes

**Files:**
- Modify: `crates/makit-tools/src/murb.rs`
- Modify: `crates/makit-tools/src/python.rs`
- Modify: `crates/makit-tools/src/murb_report.rs` (the test's `MurbResults` and `MurbInput` literals only)
- Modify: `crates/makit-tui/src/runtime.rs:570-590`, `crates/makit-tui/src/update.rs:1666-1686`, `crates/makit-tui/src/preview.rs:413-433` (literals only)
- Delete: `scripts/murb_runner.py`, `scripts/tests/test_murb_runner.py`

**Interfaces:**
- Consumes: `energy::{Inputs, WindowGroup, Outputs, simulate, EnergyError}`, `energy::weather::{Weather, parse_location}`.
- Produces:
  - `pub struct Breakdown` in `murb.rs` (`Debug, Clone, PartialEq, Serialize, Deserialize`) with 15 `Vec<f64>` fields named as the engine's outputs: `transmission_heat_losses`, `transmission_heat_gains`, `ventilation_heat_losses`, `ventilation_heat_gains`, `infiltration_heat_losses`, `infiltration_heat_gains`, `internal_gains_during_htg`, `internal_gains_during_clg`, `solar_gains_during_htg`, `solar_gains_during_clg`, `lighting_consumption`, `dhw_htg_consumption`, `plug_loads_consumption`, `electricity_consumption` and `gas_consumption`.
  - `MurbResults.breakdown: Option<Breakdown>` (optional, serde default, skipped when none). `report_path` and `report_error` are removed, and so is `MurbInput.report_dir`.
  - `pub fn simulate_with(input: &MurbInput, weather: &Weather) -> anyhow::Result<MurbResults>` and `pub fn run_simulation(input: &MurbInput) -> anyhow::Result<MurbResults>`, which reads `input.epw_path` with `Weather::read` and calls `simulate_with`.

- [ ] **Step 1: Write the failing tests** in `murb.rs`:
  - `a_run_gives_the_engines_numbers`: a `MurbInput` with `makit_defaults`'s values, `name: "check"` and `epw_path` = the synthetic EPW.
    - `run_simulation` gives `name == "check_TMY"` and `weather_file == "SYNTHETIC CALGARY"`.
    - `monthly_heating_demand` matches the fixture's `makit_defaults.outputs.heating_demand`, and `breakdown.gas_consumption` matches its `gas_consumption`, both through `assert_monthly`.
    - `tedi_kwh_m2 == ((sum of heating_demand / gfa) * 100.0).round() / 100.0`.
  - `a_missing_weather_file_names_its_path`: `epw_path: "nowhere/x.epw"` gives an error string containing `nowhere/x.epw`.
  - `a_typed_nan_is_refused` (Review Focus 4): `u_windows: f64::NAN` gives an error string containing `u_windows`.
  - `results_from_the_python_bridge_still_load` (Review Focus 5): JSON with `report_path`, `report_error` and an `input` holding `report_dir` deserializes into `MurbResults`, with `breakdown == None`.
  - Replace the three tests that read `report_dir`, `local_path` and `report_path`. The `even_glass_input` test (line 1538 area) keeps every assertion except the one on `report_dir`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools murb::`
Expected: a compile failure naming `simulate_with` and `breakdown`.

- [ ] **Step 3: Implement:**
  - `fn engine_inputs(input: &MurbInput) -> energy::Inputs` maps the fields `MurbInput` has and takes `Inputs::default()` for the rest.
  - `simulate_with` fills `MurbResults` as `murb_runner.py` did. `name` is `format!("{}_TMY", input.name)`. TEDI, TEUI and GHGI are annual sums over `gfa`, rounded to two places; the engine has already refused a `gfa` that is not positive. `weather_file` is `outputs.city`, and `breakdown` is `Some`.
  - `handle_simulate` reads the weather once, runs both the main and the even-glass run through `simulate_with`, and no longer prints a murb report path.
  - `epw_city` reads the first line and goes through `weather::parse_location`.
  - Remove `report_dir()` and `local_path()`.
  - `handle_connect` prints `✓ MURB energy model built in (ported from github.com/winterweken/murb at a1d9e81)`.
  - Rewrite the module doc comment so it describes the in-process engine.
  - In `python.rs`, the doc comment names only the IFC extractor. `repo_root` looks for `pyrevit-extension/Makit.extension/lib/analyze_ifc.py`. The hint test uses module `some_package` instead of `murb_energy_tool`. `repo_file_finds_the_runner_from_the_test_cwd` becomes `repo_file_finds_the_extractor_from_the_test_cwd`, without the runner assertion.
  - In the four `MurbResults` literals, drop `report_path` and `report_error` and add `breakdown: None`. Drop `report_dir` from the `MurbInput` literals.
  - Delete the runner and its test with `git rm`. `scripts/tests/` then disappears.

- [ ] **Step 4: Run the workspace's tests to watch them pass**

Run: `env -u MAKIT_PYTHON cargo test -q 2>&1 | tail -15`
Expected: every suite passes, with 0 failed.

- [ ] **Step 5: Check the real command end to end**

Run: `env -u PYTHONPATH -u MAKIT_PYTHON cargo run -q -p makit -- exec murb analysis murb-simulate -o epw=crates/makit-tools/src/energy/testdata/synthetic.epw -o gfa=12000 -o walls-ag=4200 -o windows=1800 -o roof=1100 -o output=$TMPDIR/makit-check.json`
Expected: a TEDI, TEUI and GHGI summary, then `Results saved to:` and `Report page:` lines, with no Python involved and no `results/` folder written.

- [ ] **Step 6: Commit**

```bash
git add -A crates/makit-tools/src crates/makit-tui/src scripts
git commit -m "Run MURB simulations in-process and drop the Python bridge"
```

### Task 7: Heat flows on the report page

**Files:**
- Modify: `crates/makit-tools/src/murb_report.rs`
- Modify: `crates/makit-tools/src/murb_report.html`

**Interfaces:**
- Consumes: `MurbResults.breakdown: Option<Breakdown>`, `MurbResults.gfa`.
- Produces:
  - `pub struct Flow { pub name: String, pub kwh_m2: f64 }` (Serialize, PartialEq, Debug, Clone), rounded to one decimal.
  - `pub struct HeatFlows { pub heating_losses: Vec<Flow>, pub heating_gains: Vec<Flow>, pub cooling_gains: Vec<Flow>, pub note: String }`.
  - `PageData.heat_flows: Option<HeatFlows>`, which is `None` without a breakdown or when `gfa <= 0`.

- [ ] **Step 1: Write the failing tests** in `murb_report.rs`:
  - `the_page_shows_where_the_heat_goes`: `arc_results()` with a `Breakdown` where every series is `vec![100.0; 12]`, except `solar_gains_during_htg` = `vec![50.0; 12]`. With `arc_results`' `gfa` called `g`, assert:
    - `heating_losses` names are `["Transmission", "Ventilation", "Infiltration"]`, each `round1(1200.0 / g)`.
    - `heating_gains` names are `["Internal gains", "Solar gains"]`, with the solar value `round1(600.0 / g)`.
    - `cooling_gains` names are `["Transmission", "Ventilation", "Infiltration", "Internal gains", "Solar gains"]`.
    - `note` equals the copy below.
  - `old_results_have_no_heat_flows` (Review Focus 5): `breakdown: None` gives `heat_flows == None`, and `render_html`'s embedded data has `"heat_flows": null`.
  - `the_caveat_names_the_engine_makit_carries`: the notes contain the new caveat below, and no note starts with `murb_energy_tool is written`.

- [ ] **Step 2: Run them to watch them fail**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools murb_report::`
Expected: a compile failure naming `heat_flows`.

- [ ] **Step 3: Implement** `page_data`'s heat flows as annual sums over GFA, in murb's report units (kWh/m² per year). Use this exact copy:
  - note: `Heating demand is worked out month by month and never goes below zero, so in mild months the gains outweigh the losses and the surplus is not carried over: these figures do not add up to the TEDI above.`
  - caveat, replacing the old one: `The energy model, ported from murb_energy_tool, is written for multi-unit residential buildings; for other building types treat these results as indicative.`

  The template gets one new `<section aria-labelledby="heat-h">` between "The year" and "The envelope". Its label `<div class="label" id="heat-h">` reads `Where the heat goes`, and its `<h2>` reads `Lost while heating, gained while cooling`. It shows the three groups as labelled horizontal bars to one shared scale in kWh/m², using `--heat` for the heating rows and `--cool` for the cooling ones, followed by the note as a paragraph. The section gets the `hidden` attribute when `D.heat_flows` is null. The layout is otherwise the builder's call, within the page's existing styles.

- [ ] **Step 4: Run the tests and look at the page**

Run: `env -u MAKIT_PYTHON cargo test -q -p makit-tools murb_report:: && cargo run -q -p makit -- exec murb analysis murb-simulate -o epw=crates/makit-tools/src/energy/testdata/synthetic.epw -o gfa=12000 -o walls-ag=4200 -o windows=1800 -o roof=1100 -o output=$TMPDIR/makit-check.json`
Expected: tests pass, and the page written beside the results, `$TMPDIR/makit-check.html`, shows the new section. Open it in the built-in browser at phone and desktop widths, in light and dark.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/murb_report.rs crates/makit-tools/src/murb_report.html
git commit -m "Show where the heat goes on the MURB report page"
```

### Task 8: Docs, the showcase and the recording

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `AGENTS.md`
- Modify: `examples/MURB/Ifc4_ARC-in-Toronto/` (regenerated; `results/` deleted)
- Modify: `docs/media/makit-tui.tape`, `docs/media/makit-tui.gif` (re-recorded)

**Interfaces:**
- Consumes: the finished feature.

- [ ] **Step 1: Regenerate the showcase and check it against the Python run.** Record the old intensities first. Then delete murb's report folder and re-run the recorded command:

```bash
git show HEAD:examples/MURB/Ifc4_ARC-in-Toronto/murb_results.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['tedi_kwh_m2'], d['teui_kwh_m2'], d['ghgi_kg_m2'], d['even_glass']['tedi_kwh_m2'])"
git rm -r -q "examples/MURB/Ifc4_ARC-in-Toronto/results"
cargo run -q -p makit -- exec murb analysis murb-simulate -o epw=examples/EPW/Toronto_TMU/CAN_ON_Toronto.City-Univ.of.Toronto.715080_TMYx.epw -o model=examples/IFC/Ifc4_ARC.ifc -o output=examples/MURB/Ifc4_ARC-in-Toronto/murb_results.json
```

Expected: the new TEDI, TEUI, GHGI and even-glass TEDI equal the old ones, at two decimals, on real Toronto weather. Any difference is a parity bug to debug before going on.

- [ ] **Step 2: Update the docs:**
  - `README.md`:
    - In the MURB row (line 29) and the `murb-simulate` table row (line 217), the model is built in.
    - Drop `PYTHONPATH=../murb` from both simulate examples (lines 89 to 110), along with the sentence about murb's own HTML report.
    - The `murb` source row (line 219) becomes "Working: reports that the energy model is built in".
    - Drop `murb_runner.py` from the tree (line 187), and make the `examples/MURB/` comment "results JSON and report page" (line 190).
  - `CLAUDE.md`: the MURB bullet says the model runs in-process in `crates/makit-tools/src/energy/`, ported from murb at `a1d9e81`, and keeps its `--model` sentence.
  - `AGENTS.md`:
    - Add `energy/` to the makit-tools module list (the murb port, one file per murb module, parity fixture in `energy/testdata`).
    - Drop `murb_runner.py` from the scripts line, and delete the MURB runner test line.
    - Set the `cargo test` counts from a real run.
    - Rewrite the MURB trap row. Its key traps:
      - Python's `%` takes the divisor's sign, so use `rem_euclid`.
      - The engine fails wherever murb raises.
      - `parity.json` is regenerated only with `generate_parity.py` against murb `a1d9e81`.
      - The synthetic EPW is test data, not weather to model a building on.
  - Run `grep -rn "PYTHONPATH\|murb_runner\|murb_energy_tool" README.md CLAUDE.md AGENTS.md docs/*.md`. Every remaining hit must describe the port's origin, nothing that needs the package.

- [ ] **Step 3: Re-record the TUI.**
  - In the tape, the hidden setup line drops `PYTHONPATH=../murb-pp`. Its header comment drops the murb checkout from "Needs".
  - Move the user's root `murb_results.json` and `murb_results.html` aside to the scratchpad, run `vhs docs/media/makit-tui.tape`, then delete the files the recording wrote in the root and move the user's back.

Run: `vhs docs/media/makit-tui.tape && ls -la docs/media/makit-tui.gif`
Expected: a fresh GIF. Open it in the built-in browser and watch it through: the picker loads the model, and the run completes with results and no Python error.

- [ ] **Step 4: Run everything**

Run: `cargo fmt --check && cargo clippy --all-targets -q && env -u MAKIT_PYTHON cargo test -q 2>&1 | grep "test result"`
Expected: no formatting diff, no clippy warnings, and every `test result: ok`. The counts match `AGENTS.md`.

- [ ] **Step 5: Commit**

```bash
git add README.md CLAUDE.md AGENTS.md docs/media examples/MURB
git commit -m "Document the built-in energy model and refresh the showcase"
```
