# The MURB energy engine in Rust, inside makit

Date: 2026-11-23. Status: draft for approval.

## Intent

`murb-simulate` should work from the makit binary alone. Today it shells out
to `scripts/murb_runner.py`, which imports the `murb_energy_tool` Python
package from a separate checkout of github.com/winterweken/murb that the user
has to put on `PYTHONPATH`. Someone who installs makit and runs the TUI gets
"no Python with murb_energy_tool found" and has no way to know why. The user
decided that makit becomes the engine's home: the engine is ported to Rust and
the Python murb is frozen as the reference the port is checked against. The
Streamlit web app in the murb repository stays on the Python engine and is not
part of this work.

## Goals

- `murb-simulate`, the TUI's energy run and `murb-report` work with no Python,
  no second checkout and no environment variable. An EPW weather file is the
  only thing a run needs besides makit, plus the IFC extractor's existing
  needs when a model has no fresh extraction.
- The Rust engine gives the same numbers as the Python port of murb at commit
  a1d9e81 (the `pure-python-port` tip, merged into murb `main` as 83cf3b8),
  checked by tests in makit.
- makit's own report page gains the thermal breakdown murb's separate HTML
  report used to give: where the heat is lost and what offsets it.
- The Python bridge is removed, not kept as a fallback.

## Non-goals

- murb's target-solving routines (`routines.py`: minimum facade and heating
  COP to meet a TEDI, TEUI or GHGI target), utility-bill validation
  (`reporting.validate`), ISD weather input, console tables and the
  interpolation helpers they use. makit does not call them today.
- murb's own HTML report (`reporting.write_results`, `skeleton.html`). It is
  replaced by the breakdown on makit's page.
- New `murb-simulate` options. The engine accepts every input murb's `Run`
  does, but the CLI and TUI keep exposing what they expose now.
- Shipping weather files to users, or removing the IfcOpenShell dependency of
  IFC extraction. Both are later work.
- Changing the murb repository. Freezing or archiving it is the user's call.

## Anti-goals

- Numbers that silently differ from murb. A user comparing makit's TEDI to
  the murb web app on the same inputs must get the same answer.
- NaN or infinity reaching a results file or the report page. The Python
  engine raises on these cases; the Rust one returns an error.
- Engine code that reads options, prints, or touches the registry. The
  engine takes plain inputs and returns plain outputs.

## Constraints

- No new external crates for the engine: plain `f64` arithmetic and `std`.
  `thiserror` (already a workspace dependency) is added to makit-tools for the
  engine's typed errors, per the project's convention for library code.
- Results files written before this change must still load in `murb-report`
  and the TUI. `MurbResults` and `MurbInput` stay serde-compatible: removed
  fields are ignored on read, new fields are optional.
- `cargo fmt` and `cargo clippy` clean; tests live in `#[cfg(test)] mod tests`
  beside the code; deterministic inputs come from files under `examples/`.

## Approaches

1. **A new pure crate for the engine.** Inputs and parsed weather in, monthly
   series out, no dependencies, like makit-geometry. Cleanest separation; one
   more crate.
2. **A module inside makit-tools.** The same port with less ceremony. The
   engine sits beside the HTTP and server code, so the separation is kept by
   convention, not by the crate boundary.
3. **Copy the Python into makit and find a Python automatically.** Smallest
   change, but users still need Python installed, which is what this work
   removes. Ruled out.

The user chose **approach 2**.

## The how

### Where it lives

A new module `crates/makit-tools/src/energy/`, declared in makit-tools'
`lib.rs`. It does not depend on makit-core, the registry, serde options or any
other makit-tools module; `murb.rs` depends on it, never the reverse. Its doc
comment names the murb commit it was ported from. Suggested files, mirroring
murb's modules so a reader can diff them side by side:

| File | Ported from | Holds |
|---|---|---|
| `mod.rs` | `simulation.py` | `Inputs`, `WindowGroup`, `Outputs`, `simulate`, the error type |
| `weather.rs` | `epw.py`, `utilities.py` | EPW parsing, monthly sums, degree hours |
| `solar.rs` | `solarpos.py`, `solargains.py` | NOAA solar position, window gains, utilisation factors |
| `heat_balance.rs` | `heatbalance.py` | transmission, ventilation, infiltration, outdoor air, internal gains |
| `end_uses.rs` | `models.py` | lighting, hot water, plug loads, heating and cooling energy |
| `constants.rs` | `static.py` | NECB, ASHRAE and emission-factor tables, provinces |

### The engine's interface

- `Weather::parse(text: &str) -> Result<Weather, EnergyError>` is pure; a
  thin `Weather::read(path)` reads the file. `Weather` carries the hourly
  columns murb reads (month, day, hour, dry-bulb, GHI, DNI, DHI), the hourly
  sun elevation and azimuth computed on parse, and the station's city from the
  `LOCATION` header. `murb::epw_city` reuses this header parsing.
- `Inputs` has one field per `Run` argument and per `Run` keyword that the
  engine honours (`clg_pct`, `mass_level`, `occupant_density`,
  `peak_receptacle_load`, `lpd`, the occupancy, lighting and receptacle
  fractions, `service_water_heating_load`, the two ASHRAE 62.1 rates), with
  murb's defaults from `Default`. `mass_level` is an enum (low, medium, high).
- `simulate(inputs: &Inputs, weather: &Weather) -> Result<Outputs, EnergyError>`.
  `Outputs` holds the 21 monthly series murb's golden tests record: heating and
  cooling demand and consumption, lighting, hot water, plug loads, total,
  electricity, gas, GHG emissions, the six transmission, ventilation and
  infiltration series, internal and solar gains during heating and cooling,
  and the weather station's city. murb's diagnostic `f` and `f_prime` are
  not needed.
- Errors, each with a message naming the problem: unknown province (listing
  the valid codes; any case is accepted), no window groups, an EPW that cannot
  be parsed (missing `LOCATION`, a short row, a field that is not a number, an
  hour outside 1 to 24, a February 29 row), an input that would divide by zero,
  and a utilisation factor that cannot be computed. The engine fails wherever
  the Python raises, so a run never succeeds in makit that murb would refuse.
  Checked up front: gross floor area, occupant density, every U-value, every
  COP and the above-grade envelope area must be positive. Checked per month:
  both utilisation-factor denominators, `heat_losses - internal_gains` and
  `1 + c*glr + d*glr^2`, must be non-zero. The second has real roots for low
  and medium mass, and summer months give negative `glr`.

### Faithfulness to the Python, and the traps in porting it

- Python's `%` always returns a value with the divisor's sign. The solar
  position takes `% 360` and `% 1440` of values that go negative for western
  longitudes. Use `rem_euclid`, never Rust's `%`.
- The EPW reader strips a UTF-8 byte order mark, accepts CRLF and LF line
  endings, replaces invalid UTF-8 rather than failing (Python reads with
  `errors='replace'`), skips blank lines, and starts data at row 9. Hours are
  stored as `hour - 1`, the start of the hour, and the year is fixed at 2021.
- The Julian day uses the proleptic Gregorian day ordinal Python's
  `date.toordinal()` gives; compute it in code, with no date crate.
- `x ** -1` in the Python is `1.0 / x` in Rust; `math.log`, `sin`, `cos`
  and the rest map to `f64` methods. Results may differ from the Python in
  the last bits; the tests allow for that and no more.
- `f64` division by zero gives infinity or NaN instead of raising, and some
  of those infinities turn back into finite numbers: a zero U-value gives an
  air-to-air U of zero. A final NaN check alone would therefore let through
  runs murb refuses; the checks listed under errors are what keeps parity.
- Gas carries heating when `cop_htg < 0.99` and hot water when
  `cop_dhw < 0.99`; otherwise they go to electricity. Keep these thresholds.
- The run's name gets `_TMY` appended, as murb does. makit's page strips it
  for its title; the CLI summary and the TUI's Output pane show it as they do
  today.

### Tests

- **Parity with the Python port.** A fixture file holds, for each of murb's
  six golden scenarios (`baseline_gas`, `four_orient`, `all_electric_hp`,
  `low_mass`, `half_cooled`, `below_grade`) plus one scenario with makit's own
  defaults (Ontario, makit's U-values and COPs, a quarter of the glass each
  way), every output series as the Python port computes it on the Calgary
  CWEC weather file. The Rust engine must match each value to a relative
  tolerance of 1e-9 with an absolute floor of 1e-9 kWh.
- **The original goldens.** murb's `goldens.json`, made from the numpy and
  pvlib engine, is also checked at the tolerances murb's own parity test uses
  (1 % annual and 2 % or 50 kWh monthly for series touched by solar gains,
  1e-4 relative for the rest).
- **Unit tests** for the pieces whose behaviour the parity runs do not pin:
  the solar position at a few known instants including a negative true solar
  time, EPW parsing of a BOM, CRLF, a short row and a February 29 row, each
  error, and lower-case provinces.
- The weather file goes in `examples/EPW/CAN_AB_CALGARY-INTL-A_3031092_CWEC.epw`
  with a short README in `examples/EPW/` giving its source and licence. It
  is the CWEC 2011 file for Calgary International (station 718770), about
  1.6 MB. It reached the murb repository with the original import from
  KPMB-LAB, so its source has to be established on its own. CWEC data are
  published by Environment and Climate Change Canada; confirm the licence on
  the publisher's page and quote it in that README before committing the file.
  *Decided in review:* if the licence cannot be confirmed, the tests run on a
  synthetic weather file instead, generated by a committed script with
  deterministic temperatures and irradiance at Calgary's location and time
  zone. The Python port then generates the parity fixture on that file, and
  the check against the original goldens is dropped: murb's own tests already
  tie the Python port to them.
  *Outcome of the check, 2026-11-23:* the Government of Canada's open data
  record for CWEC (Natural Resources Canada) names the Open Government
  Licence - Canada, but it covers the current 2023 edition; nothing confirms
  the licence of the 2011 file murb carries. The synthetic fallback applies,
  and its files live beside the engine's tests rather than in `examples/EPW/`,
  since synthetic weather is not something to run a real building on. The fixture JSON
  files live beside the engine's tests, with the Python script that generated
  the parity fixture and the murb commit it ran against, so the fixture can be
  regenerated if murb ever changes.

### makit's side

- `murb::run_simulation` builds `Inputs` from `MurbInput`, reads the weather,
  calls `simulate` and fills `MurbResults` exactly as the runner did: the
  monthly series, TEDI, TEUI and GHGI as annual sums over GFA rounded to two
  places, the GFA and the weather station's city.
- `MurbResults` gains an optional `breakdown` holding the monthly
  transmission, ventilation and infiltration losses and gains, internal and
  solar gains during heating and cooling, lighting, hot water, plug loads,
  electricity and gas. `report_path` and `report_error` are removed, and so is
  `MurbInput.report_dir`. The struct literals in the TUI's runtime and tests
  are updated to match.
- `handle_simulate` stops printing murb's report path. The `murb` source's
  connect check reports that the energy engine is built in.
- The report page gets a section with the same quantities murb's report
  table gave, per square metre of GFA over the year: heat lost by
  transmission, ventilation and infiltration, and heat gained by
  transmission, ventilation, infiltration, internal gains and solar gains,
  for the heating and the cooling periods. *Decided in review:* the figures are
  shown as murb computes them, without implying they sum to TEDI. They do
  not: heating demand is clamped at zero month by month, so in mild months
  gains exceed losses and the excess does not reduce the year's heating
  (on Toronto weather with makit's defaults, the year's heating demand
  is about 9 % above losses minus gains). A line on the page says so.
  `page_data` derives the section from `breakdown`; it is absent for results
  without one. The page's
  existing caveat that murb is written for multi-unit residential buildings
  stays, reworded to name the engine makit now carries.
- Removed: `scripts/murb_runner.py`, `scripts/tests/test_murb_runner.py`,
  murb's entry in `python.rs` (its hint and test). `python::repo_root` looks
  for the IFC extractor (`pyrevit-extension/Makit.extension/lib/analyze_ifc.py`)
  instead of the deleted runner, since IFC extraction still needs it.
- Docs: README (the MURB rows, the `PYTHONPATH` examples, the source table,
  the repository tree), `CLAUDE.md` (the MURB bridge bullet), `AGENTS.md`
  (module list, the scripts line, the runner test line, test counts, the
  MURB trap row), and `docs/IFC_SUPPORT.md` where it shows a simulate command.
- Showcase, *decided in review:* the committed example
  `examples/MURB/Ifc4_ARC-in-Toronto/` is regenerated with the Rust engine, so
  it carries the breakdown and no longer ships murb's own report folder. It
  needs the local `Ifc4_ARC.ifc` and the Toronto TMYx weather file, which
  exist in the main checkout only; the weather file stays uncommitted.
  `docs/media/makit-tui.tape` drops its `PYTHONPATH` export, and the GIF it
  records is re-recorded.

## Ownership and licence

makit is public under the MIT licence. murb's code reached
github.com/winterweken/murb in its first commit as an import of
`KPMB-LAB/murb_energy_tool`, a private repository, and neither carries a
licence. `static.py` credits Jonathan Graham for the averaged schedules.
Porting the engine into makit republishes that model under MIT.

*Decided by the user:* the engine is taken from the user's own repository,
github.com/winterweken/murb, at commit a1d9e81. The module's doc comment
names that repository and commit as its source, and the credits in murb's
source comments, such as the averaged schedules credited to Jonathan Graham,
are carried over with the constants they annotate.

## Left to the builder

- Names inside the engine beyond those above, and whether `Outputs` is one
  struct or grouped sub-structs.
- How the page lays out the breakdown, within the page's existing style.
- Whether the even-glass comparison run reuses the parsed weather rather than
  reading the file twice (it should, but either is correct).
- The order of the work, as long as parity tests exist before the Python
  bridge is deleted.
