# IFC envelope for MURB — design

Date: 2026-11-22. Status: draft for approval (builder-checked; the calls it
raised are settled inline in §2, §4 and §7).

## Intent

Run a MURB energy simulation from an IFC file and a weather file, with nothing
else typed:

```bash
makit exec murb analysis murb-simulate -o model=house.ifc -o epw=toronto.epw
```

In the TUI: pick the IFC, give an EPW, press `e`.

Today only the window split by facing comes from the model. GFA, above-grade
wall area, window area and roof area are required typed options, below-grade
walls have no source at all, and the IFC must first be extracted by hand with
a Python command that makit prints. The extractor reads walls and windows only,
so none of the missing quantities can be derived from an extraction yet.

The why: makit's job is to bridge BIM models and analysis from one terminal.
Hand-measuring envelope areas out of a model defeats that, and early-stage
energy modelling (what MURB is for) is exactly where a quick, model-derived
first answer matters.

## Goals

1. Every envelope quantity MURB takes comes from the IFC when not typed: GFA,
   above-grade wall area, below-grade wall area, window area, roof area, and
   the window split by facing (already there).
2. makit runs the IFC extractor itself when an IFC has no extraction, or a
   stale one (the IFC is newer than its JSON, or the JSON predates this
   format), finding a Python with IfcOpenShell on its own. This applies to
   `murb-simulate --model`, `makit analyze`, `wall-orientation-wwr --input`,
   the TUI picker, and a new `makit extract` command / the `ifc` source.
3. Every derived number is visible before it is used: GFA by storey with slab
   counts, walls above/below grade with the grade used, what was left out and
   why. A typed option always wins over the model.
4. The three local sample files extract and simulate: the SketchUp house
   (IFC4, quantities, everything flagged external), the Duplex (IFC2x3, no
   quantities at all, overlapping finish floors, windows with overall sizes
   only) and the Revit sample house (IFC4, Revit quantities, split levels,
   building pads and pile caps exported as slabs).

## Non-goals

- Parsing IFC in Rust. IfcOpenShell stays the geometry engine.
- MURB's slab-on-grade inputs (`area_sog`, `perim_exp`). The runner does not
  pass them today; the extraction will carry what is needed to add them later
  (the lowest plate's footprint and its perimeter), but they stay at MURB's
  defaults in this change.
- Slabs, storeys or exterior flags from the live Revit (pyRevit) extractor.
- Classifying slabs by name ("Pad", "Finish", "Exterior"). Only IFC types,
  aggregation and geometry are used.
- A review UI for slabs in the TUI beyond the summary lines.

## Anti-goals

- A GFA that looks authoritative but is wrong. The per-storey breakdown is
  always printed next to the number, and the model never overrides a typed
  value.
- Making today's path worse. A fresh JSON beside an IFC is used without
  Python. A stale JSON is used with a note when extraction is impossible.
  Only a *missing* extraction with no usable Python is an error, and that
  error says how to set Python up.
- Half-written extractions. Two makit threads (the picker and a task) may
  extract the same file at once; a reader must never see a partial JSON.

## Constraints

- Extraction needs Python 3 with IfcOpenShell 0.8 or newer; `shapely` comes
  with it and is used for footprint unions. The shared `lib/` folder is also
  loaded by pyRevit's IronPython, so new Python keeps the folder's existing
  style (no f-strings, `object` base classes) and imports shapely lazily with
  a sum-of-areas fallback and a warning.
- The three-layer architecture holds: the JSON is a geometry format (Layer 2);
  analysis choices (grade elevation, which quantities to use, overrides) live
  in Rust (Layer 3). The extractor reduces geometry (footprints, unions,
  up-facing areas) but makes no energy-modelling decisions.
- The JSON format stays backward compatible: extractions without storeys or
  slabs still load for WWR; the envelope then reports what is missing.
- TUI rules from `AGENTS.md`: `update` stays pure, extraction runs on the
  worker thread, status comes back as messages, no `println!` in TUI code.
- `cargo fmt`, `cargo clippy`, `cargo test`, the Python extractor tests and
  the MURB runner tests all pass.

## Approaches

**A. Shell out to the Python extractor; compute the envelope in Rust from new
geometry facts (chosen).** The extractor grows storeys, floor plates, roofs,
wall elevations and geometry-measured windows. Rust finds Python, runs the
extractor when needed, derives the envelope, and `murb-simulate` fills what
was not typed. Same Python dependency as today, found automatically; the JSON
stays geometry; grade and overrides never need re-extraction; the envelope
logic is testable in Rust against hand-built models.

**B. The extractor computes the envelope and writes an `envelope` block; Rust
copies it.** Less Rust, but the grade elevation and the choice of quantities
get baked into each extraction, changing them means re-extracting, the
analysis logic splits across two languages, and Rust cannot test it.

**C. Parse IFC natively in Rust.** Removes the Python dependency, but IFC
geometry (extrusions, booleans, mapped items, B-reps) is what IfcOpenShell
and OpenCASCADE exist for. Weeks of work for no change in the answer.

The user chose to have makit run the extractor and left the how to me; A is
my pick for the reasons above.

## The how

### 1. Extractor (`pyrevit-extension/Makit.extension/lib/`)

New generic types in `geometry_models.py`, serialised next to `walls` and
`windows`:

- `storeys`: one per `IfcBuildingStorey`: `id`, `name`, `elevation` (m),
  `floorArea` (m², the footprint union of its floor plates), `slabIds`.
- `slabs`: `id`, `name`, `kind` (`floor`, `roof`, `landing`, `other`),
  `area` (m²), `top` and `bottom` (m, project z), `storey` (name, or null),
  `type` (ObjectType), `isExternal`, `properties` (`ifcType`).

Rules, all from IFC types, aggregation and geometry:

- A **floor plate** is an `IfcSlab` whose PredefinedType is not `ROOF` or
  `LANDING` and that is not decomposed from an `IfcRoof`, `IfcStair` or
  `IfcRamp`. Its `area` is the footprint of its up-facing faces (normal
  z > 0.5) projected to plan. It is assigned to the storey whose elevation is
  nearest its top face, within 0.3 m (finish floors sit a few centimetres
  above the level, structural slabs at it). A plate with no storey within
  0.3 m keeps `storey: null` and is not counted; Rust reports it. This is
  what drops the Revit sample's building pads buried under its floors and the
  pile stubs, while keeping its 'Foundation Slab' that shares the Level 2
  plate.
- A storey's `floorArea` is the **union** of its plates' footprints, not the
  sum, so finish floors over structural floors count once.
- A **roof** is one `IfcRoof` (its own geometry when it has a representation,
  else its decomposed parts, never both), or an `IfcSlab` of type `ROOF` that
  is not under an `IfcRoof`. Its `area` is the up-facing surface area
  (normal z > 0.5), so a gable counts its slopes, not its footprint.
- Geometry comes out of `ifcopenshell.geom` with world coordinates in metres;
  storey elevations and quantities are scaled by the file's units as today.
- Walls gain `bottom` and `top` (m) from their geometry's z range; without
  geometry, from the placement z and the Height quantity; null when neither
  exists.
- Windows take their area from quantities, else `OverallHeight × OverallWidth`
  (scaled), else their geometry (the existing one-side measurement). This
  fixes the Duplex, whose 24 windows extract at 0 m² today.
- `metadata.formatVersion` is written as `2`. Rust treats a missing version as
  `1`.
- `analyze_ifc.py --extract-only` reports storeys and slabs in its summary.

### 2. Rust: python discovery and extraction (`makit-tools`)

New `python.rs`:

- `find(module) -> Result<PathBuf>`: `$MAKIT_PYTHON` if set (it must import
  `module`; if it cannot, that is the error, no silent fallback), else
  `<repo>/.venv/bin/python` (`Scripts\python.exe` on Windows), else `python3`,
  then `python` on PATH; the first whose `-c "import <module>"` succeeds. The
  error lists what was tried and the README's setup command.
- `repo_file(relative) -> Result<PathBuf>`: locates repository files by
  walking up from the executable's folder and from the working directory,
  replacing `murb.rs`'s `find_runner_script`. `murb.rs` switches to
  `find("murb_energy_tool")` and `repo_file("scripts/murb_runner.py")`.

`ifc.rs` (today a placeholder source) becomes the extraction module:

- `extract(ifc, output, force) -> Result<PathBuf>`: runs
  `analyze_ifc.py <ifc> --extract-only --output <tmp>` with the found Python,
  then renames the temp file into place (atomic, so readers never see a
  partial file). Output defaults to `extraction_path(ifc)` (`<name>.json`
  beside it). Failure returns the extractor's stderr.
- `extraction_for(ifc) -> Result<Resolved>`, with
  `Resolved { json, extracted: bool, note: Option<String> }`: finds the JSON
  with `find_json_for_ifc`; missing or stale → extract. Stale means the IFC's
  mtime is more than 2 s newer than the JSON's (a git checkout writes both
  within milliseconds, in either order, and must not count), or
  `formatVersion < 2`. Stale and extraction fails → the old JSON with a note
  saying why. Missing and extraction fails → error with the setup hint. The
  staleness/extract decision is a pure function over (found, stale,
  extract-result) so it is unit-tested without Python.
- A cancelled task (`ctx.cancel`) kills the Python child; the temp output is
  removed and the old JSON, if any, is left alone.
- The `ifc` source handler: `--file` (required, the picker fills it),
  `--output`, `--force`; runs `extract`, prints the JSON path and a one-line
  summary (walls, windows, slabs, storeys). The extractor's own stdout is
  inherited so progress and warnings land in the Output pane or terminal.

### 3. Rust: the envelope (`building_model.rs`)

Model additions: `storeys: Vec<Storey>`, `slabs: Vec<Slab>` (kind as an
enum), wall `bottom`/`top: Option<f64>`, `metadata.format_version`.

`envelope(model, grade) -> Envelope` where grade is a project-z elevation in
metres (default 0):

- `gfa` = Σ `storeys[].floor_area`, with each storey's name, elevation, area
  and plate count kept for display.
- `roof` = Σ roof slabs' `area`, each kept for display.
- Walls: the envelope is every wall not flagged interior (as `analyze_wwr`).
  Each wall's net `area` splits by the fraction of its `[bottom, top]` below
  `grade`; a wall without elevations counts above grade and is tallied in
  `walls_without_height`. A curtain wall's windows stay above grade.
- `windows` = Σ window area in envelope walls plus windows in no wall (as
  `analyze_wwr`); `windows_by_facing` reuses `window_area_by_facing`.
- `left_out`: plates with no storey (name, area), and the count of storeys
  with no plate.
- `print_envelope` renders the table used by `makit analyze` and the murb
  handler.

### 4. `murb-simulate`

- `gfa`, `walls-ag`, `windows`, `roof` become optional in the registry;
  `walls-bg` stays optional; new `grade` (float, default `0`, "Grade
  elevation [m] in project coordinates; wall area below it counts as
  below-grade"). `model`'s description becomes "IFC file or extraction:
  envelope areas and window directions come from it".
- Resolution per quantity: typed value → model envelope → error naming the
  option ("gfa: give --gfa or --model"). A typed `windows` with a model still
  takes its directions from the model, as today.
- A zero from the model is a gap for `gfa`, `walls-ag` and `roof` (a building
  has all three; MURB divides by GFA): the error names the option and says the
  model lacks it ("roof: the model has no roof; give --roof"; for a format-1
  extraction, that it predates storeys and should be re-extracted). A zero is
  a value for `windows` and `walls-bg`: the SketchUp house has no windows and
  must simulate. A model with no windows gives the even 25 % split instead of
  today's error in `window_groups_from_model`.
- `walls-bg` loses its registered default of `0`. The TUI lays registered
  defaults over a run's options, so a default would reach the handler as if
  typed and mask the model's value. The handler uses 0 when neither typed nor
  in the model. `grade` keeps its default; it is never model-sourced.
- A `.ifc` model goes through `extraction_for` (so it extracts on demand); a
  `.json` loads directly.
- Before running, the handler prints the envelope with its sources: each
  quantity marked "typed" or "from <json>", the storey breakdown, the grade,
  and the notes.

### 5. CLI

- `makit analyze x.ifc` uses `extraction_for`, prints "Extracted to x.json"
  or the stale note, and adds the envelope to the default summary.
  `-t envelope` prints the envelope alone. New `--grade` argument (default 0).
- New `makit extract <file.ifc> [--output path] [--force]` calling
  `ifc::extract`. Sources are not reachable from `makit exec` today and the
  `exec tool category task` shape does not fit them; a subcommand is the
  honest CLI face.
- `wall-orientation-wwr --input x.ifc` uses `extraction_for`, closing the
  "given an .ifc path it fails" gap in the README's status table.

### 6. TUI

- `runtime::load_ifc` calls `extraction_for`. Before running Python it sends
  a new `Msg::IfcExtracting { path }`; `update` sets the status to
  "◌ Extracting <name>…" only while that path is still the selection.
- `IfcOutcome::Analysis` gains `extracted: bool` and `note: Option<String>`;
  the status line says "extracted to <name>.json" when it did, and shows the
  note when the JSON was stale but used.
- `IfcOutcome::NeedsExtraction` gains `reason: String` (why extraction could
  not happen); the status and the picker detail show it in place of today's
  "Not extracted yet: no X beside it".
- The picker detail shows one envelope line under the WWR summary when the
  extraction carries storeys: "GFA 299 m² · walls 423 / 61 m² · windows 37 m²
  · roof 137 m²" (the layout is the builder's call; `layout::compute` stays
  the single source for both drawing and hit-tests).
- When the `ifc` source task finishes successfully on the picker's current
  file, `update` reissues `Cmd::LoadIfc` for it so the preview refreshes.

### 7. Docs and examples

- Regenerate `examples/IFC/building-model.json` in the new format, and commit
  the extractions of the two local samples, `examples/IFC/Ifc4_ARC.json`
  (83 KB) and `examples/IFC/Ifc2x3_Duplex_Architecture.json` (63 KB). The IFC
  files themselves stay out of the repository as today (`*.ifc` is ignored).
  With the extractions in, the expected-numbers table below becomes Rust
  tests over real models instead of a by-hand check, and a fresh clone can
  `makit analyze` all three without Python.
- `examples/EPW/` stays untracked (third-party weather data; committing it is
  the user's call). The README's examples keep a placeholder EPW path; the
  end-to-end verification uses the local Toronto file.
- README: features row for IFC, the CLI examples (`makit extract`, a
  `murb-simulate` with only `model` and `epw`), the Module Status rows for
  the `ifc` source, `wall-orientation-wwr` and `murb-simulate`, and
  `MAKIT_PYTHON` under Development. The hand-typed murb example (GFA 335.5,
  walls 307.4) is replaced by what the model gives; its roof (244.3) and
  windows (182.8) already match the model.
- `AGENTS.md`: `makit extract` in commands, `python.rs` in the module list,
  and a trap-table row for the GFA and grade rules.
- `docs/IFC_SUPPORT.md` and `examples/IFC/README.md`: the new fields, the
  plate and roof rules, and that extraction is automatic.

### 8. Tests

- Python (`pyrevit-extension/tests`): synthetic files built with
  `ifcopenshell.api`: two overlapping floors on one storey count once; a slab
  0.5 m under a level gets no storey; a roof aggregate of two sloped slabs
  measures its slopes once; a `ROOF` slab outside any `IfcRoof` counts; a
  stair landing does not; wall `bottom`/`top` from geometry and from
  placement + Height; window area from `OverallHeight × OverallWidth` in a
  millimetre file and from geometry. Sample-file test on
  `Building-Architecture.ifc`: one storey, GFA 25.8 m², roof 53.6 m².
- Rust (`building_model`, `ifc`, `murb`): envelope on hand-built models
  (grade split of a wall from −1.5 to 2 m at grade 0 is 43 % below; walls
  without heights; loose windows; a format-1 model gives zero GFA and a
  note); the pure staleness decision; python discovery with
  `MAKIT_PYTHON` pointing at an interpreter without the module; murb option
  resolution (typed wins, model fills, a missing or zero `gfa`/`walls-ag`/
  `roof` names the option, a model without windows gives the even split);
  envelope figures of the three committed sample extractions against the
  table below.
- TUI: `IfcExtracting` status, `NeedsExtraction { reason }` display, reload
  after the `ifc` source finishes.
- End to end, by hand and recorded in the PR: `makit extract` and
  `makit analyze` on the three samples, then `murb-simulate` on each with
  the Toronto EPW. The simulation needs `murb_energy_tool` importable; the
  local checkout needs numpy, scipy, pandas and pvlib, which go into `.venv`
  for verification only.

Expected envelope numbers from the probes done for this design, to check
the implementation against:

| Sample | GFA | Roof | Exterior walls (net) | Windows |
|---|---|---|---|---|
| SketchUp house | 25.8 (1 storey) | 53.6 | 36.4, of which 3.0 below grade 0 | none |
| Duplex | 299.3 (180.5 + 118.7) | ~133–137 | 422.7 | 24 windows, area from overall sizes |
| Revit sample house | 718.8 over 5 storeys, 'Ceiling' 149.5 and 'Foundation' 3.6 included; 'Pad:Pad 2' (287.3) and 9 pile stubs left out | 244.3 | 707.2, ≈180 below grade 0 | 191.9 |

The Revit sample is the messy case the breakdown exists for: its 'Ceiling'
plate is a building pad under Level 2 and 'Foundation' is pile caps. Both are
visible in the breakdown and overridable with `--gfa`.

## Left to the builder

- Exact wording of status lines, notes and the envelope table layout.
- Whether the Python probe result is cached per process.
- Test fixture construction details and tolerances.
- Column layout of the picker's envelope line on narrow panes.
