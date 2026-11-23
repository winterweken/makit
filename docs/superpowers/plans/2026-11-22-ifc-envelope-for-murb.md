# IFC Envelope for MURB Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `murb-simulate -o model=house.ifc -o epw=toronto.epw` runs with nothing else typed: makit extracts the IFC itself and derives GFA, wall areas split at grade, window and roof areas from it.

**Architecture:** The Python extractor (`pyrevit-extension/Makit.extension/lib/`) grows storeys, floor plates, roofs, wall elevations and geometry-measured windows, all geometry facts. Rust (`makit-tools`) finds a Python with IfcOpenShell, runs the extractor when the JSON beside an IFC is missing or stale, and computes the MURB envelope from the JSON; `murb-simulate`, `makit analyze`, `makit extract`, `wall-orientation-wwr` and the TUI picker all go through one `ifc::extraction_for` entry point.

**Tech Stack:** Rust 2021 (serde, anyhow, std::process), Python 3 + IfcOpenShell 0.8+ (shapely comes with it), ratatui TUI in Elm style.

**Spec:** `docs/superpowers/specs/2026-11-22-ifc-envelope-for-murb-design.md`

**Branching:** work on a feature branch cut from `main` (f349c3c, which equals `origin/dev`; the local `dev` is behind). PRs target `dev`.

## Global Constraints

- Python in `pyrevit-extension/Makit.extension/lib/` keeps the folder's style: no f-strings, `object` base classes, `"{}".format`; shapely is imported lazily inside the function that uses it, with a sum-of-areas fallback and a printed warning.
- The JSON stays backward compatible: every new key is optional on read (`#[serde(default)]` in Rust, `.get` in Python). `metadata.formatVersion` is `2`; a missing version reads as `1`.
- Floor plates attach to the storey whose elevation is within `0.3 m` of their top face; roofs count up-facing faces with normal `z > 0.5`.
- Staleness: the IFC's mtime is more than `2 s` newer than the JSON's, or `formatVersion < 2`.
- A zero from the model is a gap for `gfa`, `walls-ag`, `roof`; a value for `windows`, `walls-bg`. Grade defaults to `0` (project z, metres).
- `walls-bg` has no registered default; `grade` keeps `0`.
- Python discovery order: `$MAKIT_PYTHON` (must import the module, no fallback), `<repo>/.venv/bin/python` (`Scripts\python.exe` on Windows), `python3`, `python`.
- TUI: `update` stays pure; extraction runs on the IFC worker thread; status only via `Msg`; no `println!` in `makit-tui`.
- Before every commit: `cargo fmt && cargo clippy` clean, `cargo test`, and for Python tasks `.venv/bin/python -m unittest discover -s pyrevit-extension/tests`.
- Commit messages: short imperative summaries, ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Review Focus

Inputs the spec implies but did not single out; each has its test added to the owning task.

1. An IFC with no `IfcBuildingStorey` at all: every plate gets `storey: null`, GFA is 0, and `murb-simulate` must say "gfa: the model has no storeys" rather than run. (Task 1 Python test, Task 3 and Task 6 Rust tests.)
2. A wall with zero or inverted height (`top <= bottom`) must count fully above grade, never divide by zero or go negative. (Task 3 `below_grade_fraction` test.)
3. The extractor fails after a stale JSON exists: the old JSON is used with a note and the temp file is gone; the picker and `murb-simulate` keep working. (Task 5 `resolve` test and `extract` temp-file test.)
4. A bare filename (`house.ifc`, no directory) and a JSON given as `--model` must both resolve without touching Python. (Task 5 `extraction_for` test, Task 6 `.json` model test.)
5. A file in feet (US exports): storey elevations and wall elevations must come out in metres. (Task 2 Python test with a `FOOT`-unit file.)

---

### Task 1: Storeys, floor plates and roofs in the Python extractor

**Files:**
- Modify: `pyrevit-extension/Makit.extension/lib/geometry_models.py` (add classes after `GenericWindow`, extend `BuildingModel` at lines 139–166)
- Modify: `pyrevit-extension/Makit.extension/lib/ifc_extractors.py` (new functions after `get_triangles` at line 413; call from `extract_building_model_from_ifc` at line 599)
- Test: `pyrevit-extension/tests/test_ifc_extractors.py`

**Interfaces:**
- Consumes: existing `get_triangles(element)` → list of `(unit_normal, area, centroid)`; `get_unit_scales(ifc_file)`; `get_storey_name`.
- Produces (Python):
  - `geometry_models.FORMAT_VERSION = 2`
  - `class GenericStorey(id, name=None)` with `elevation` (m), `floor_area` (m²), `slab_ids` (list); `to_dict()` → keys `id, name, elevation, floorArea, slabIds`; `from_dict(data)`.
  - `class GenericSlab(id, name=None)` with `kind` (`'floor'|'roof'|'landing'|'other'`), `area` (m²), `top`, `bottom` (m or None), `storey` (name or None), `slab_type`, `is_external`, `properties`; `to_dict()` → keys `id, name, kind, area, top, bottom, storey, type, isExternal, properties`; `from_dict(data)`.
  - `BuildingModel.storeys`, `BuildingModel.slabs` lists, serialised as `storeys` and `slabs`; `BuildingModel.to_dict()` writes `metadata['formatVersion'] = FORMAT_VERSION` into a copy of the metadata.
  - `ifc_extractors.PLATE_STOREY_TOLERANCE = 0.3`
  - `get_footprint(triangles)` → shapely geometry of up-facing triangles (normal z > 0.5) projected to XY, or None (no shapely, or no up-facing faces).
  - `get_up_facing_area(triangles)` → float m².
  - `get_z_range(triangles)` → `(bottom, top)` or `(None, None)`.
  - `get_storey_elevation(storey, ifc_file)` → metres: `storey.Elevation * length_scale`, falling back to the placement's z.
  - `slab_kind(slab)` → `'roof'` (PredefinedType `ROOF`, or aggregated under `IfcRoof`), `'landing'` (`LANDING`, or under `IfcStair`/`IfcRamp`), `'other'` (under any other aggregate), else `'floor'`.
  - `extract_ifc_slab_to_generic(slab, ifc_file, storeys)` → `GenericSlab` or None without geometry. `storeys` is a list of `GenericStorey`; a floor plate's `area` is its footprint area and its `storey` the nearest storey by top elevation within `PLATE_STOREY_TOLERANCE`, else None.
  - `extract_ifc_roof_to_generic(roof, ifc_file)` → `GenericSlab(kind='roof')` with `area` = up-facing area of the roof's own geometry when it has a `Representation`, else of its decomposed parts; None when nothing has geometry.
  - `extract_storeys_and_slabs(ifc_file, building_model, storey_filter=None)`: fills `building_model.storeys` and `building_model.slabs` (floor plates, landings, others from `IfcSlab`; roofs from `IfcRoof` plus `ROOF` slabs not under an `IfcRoof`, each counted once), then sets each storey's `floor_area` to the footprint union of its plates (`shapely.ops.unary_union`), or the sum with a warning when shapely is missing. With `storey_filter`, only that storey and its plates are kept.

- [ ] **Step 1: Write the failing tests**

Add to `test_ifc_extractors.py`, reusing `new_file`, `place`, and the `ifcopenshell.api.context`/`geometry` pattern from `test_a_wall_without_quantities_is_measured_from_its_geometry`:

```python
def slab_box(f, body, name, origin, size, z, thickness=0.2, predefined=None):
    """An IfcSlab whose top face is at `z`, `size` = (dx, dy) metres."""
    ...  # IfcSlab with place(f, origin, (1, 0)), ObjectPlacement z = z - thickness,
         # add_wall_representation(length=dx, height=thickness, thickness=dy) rotated flat,
         # or a profile extrusion; assign_representation; PredefinedType=predefined

@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class FloorPlateTest(unittest.TestCase):
    def test_overlapping_floors_on_one_storey_count_once(self):
        # a 10x10 structural slab topped at 0.0 and a 4x4 finish floor topped at 0.02
        model = extract(...)
        level = model.storeys[0]
        self.assertAlmostEqual(level.floor_area, 100.0, places=1)
        self.assertEqual(len(level.slab_ids), 2)

    def test_a_slab_far_below_its_level_has_no_storey(self):
        # storey at 0.0, slab topped at -0.5
        self.assertIsNone(slab.storey)
        self.assertAlmostEqual(model.storeys[0].floor_area, 0.0)

    def test_without_storeys_every_plate_is_loose(self):  # Review Focus 1
        self.assertEqual(model.storeys, [])
        self.assertTrue(all(s.storey is None for s in model.slabs))

    def test_a_stair_landing_is_not_a_plate(self):
        self.assertEqual(landing.kind, 'landing')
        self.assertNotIn(landing.id, model.storeys[0].slab_ids)

@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class RoofTest(unittest.TestCase):
    def test_a_roof_aggregate_counts_its_slopes_once(self):
        # IfcRoof without geometry aggregating two 5x10 slabs tilted 30°: up-facing area = 2 * 50 / cos(30°)
        roof = next(s for s in model.slabs if s.kind == 'roof')
        self.assertAlmostEqual(roof.area, 115.5, places=0)
        self.assertEqual(sum(1 for s in model.slabs if s.kind == 'roof'), 1)

    def test_a_roof_slab_outside_any_roof_counts(self):
        # flat IfcSlab PredefinedType ROOF, 8x8
        self.assertAlmostEqual(roof.area, 64.0, places=1)

    def test_a_roof_slab_is_not_a_floor_plate(self):
        self.assertEqual(model.storeys[0].floor_area, 0.0)
```

Also a `GenericStorey`/`GenericSlab` round-trip test (`to_dict` → `from_dict`) and `test_to_dict_stamps_format_version_2`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/bin/python -m unittest pyrevit-extension.tests.test_ifc_extractors -k Plate -k Roof -k format_version 2>&1 | tail -5`
(or `cd pyrevit-extension/tests && ../../.venv/bin/python -m unittest test_ifc_extractors.FloorPlateTest test_ifc_extractors.RoofTest`)
Expected: FAIL / ERROR with `AttributeError: module 'ifc_extractors' has no attribute 'extract_storeys_and_slabs'` and `BuildingModel has no attribute 'storeys'`.

- [ ] **Step 3: Implement the models in `geometry_models.py`**

`GenericStorey`, `GenericSlab`, the `BuildingModel` fields and `FORMAT_VERSION` as in Interfaces. `to_dict` copies `self.metadata` before adding `formatVersion`.

- [ ] **Step 4: Implement the extraction functions in `ifc_extractors.py`**

The functions in Interfaces, placed after `get_triangles`. `get_footprint` imports shapely inside the function:

```python
try:
    from shapely.geometry import Polygon
    from shapely.ops import unary_union
except ImportError:
    return None
```

`extract_storeys_and_slabs` is called from `extract_building_model_from_ifc` after the windows block, passing `storey_filter`. When any plate has a footprint but shapely is missing, print one warning (`"Warning: shapely not available, storey floor areas are sums, not unions"`) and sum the plates' areas.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -3`
Expected: `OK` with the previous 22 tests plus the new ones.

- [ ] **Step 6: Commit**

```bash
git add pyrevit-extension/Makit.extension/lib/geometry_models.py pyrevit-extension/Makit.extension/lib/ifc_extractors.py pyrevit-extension/tests/test_ifc_extractors.py
git commit -m "Extract storeys, floor plates and roofs from IFC"
```

---

### Task 2: Wall elevations, window geometry fallback, and the sample extractions

**Files:**
- Modify: `pyrevit-extension/Makit.extension/lib/geometry_models.py` (`GenericWall`: `bottom`, `top`)
- Modify: `pyrevit-extension/Makit.extension/lib/ifc_extractors.py` (`extract_ifc_wall_to_generic` at line 324, `extract_ifc_curtain_wall_to_generic` at line 453, `extract_ifc_window_to_generic` at line 529)
- Modify: `pyrevit-extension/Makit.extension/lib/analyze_ifc.py:84-94` (summary line)
- Modify: `examples/IFC/building-model.json` (regenerate), create `examples/IFC/Ifc4_ARC.json`, `examples/IFC/Ifc2x3_Duplex_Architecture.json`
- Test: `pyrevit-extension/tests/test_ifc_extractors.py`

**Interfaces:**
- Produces: `GenericWall.bottom`, `GenericWall.top` (m or None; JSON keys `bottom`, `top`); `get_wall_elevations(wall, ifc_file)` → `(bottom, top)` from the geometry z range of the wall (or of its decomposed parts for a curtain wall), else placement z and the `Height` quantity scaled to metres, else `(None, None)`. Window area order: quantities → `OverallHeight × OverallWidth × length_scale²` → `get_side_area_from_geometry(window)`.
- The committed sample JSONs are what Task 3's Rust tests read; their figures must match the spec table (GFA 25.8 / 299.3 / 718.8; roof 53.6 / ~133–137 / 244.3).

- [ ] **Step 1: Write the failing tests**

```python
@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WallElevationTest(unittest.TestCase):
    def test_elevations_come_from_geometry(self):
        # wall with body geometry from z=-1.5 to 2.0 in a METRE file
        self.assertAlmostEqual(wall.bottom, -1.5, places=2)
        self.assertAlmostEqual(wall.top, 2.0, places=2)

    def test_elevations_fall_back_to_placement_and_height(self):
        # no representation; placement z = 3000 mm, Height quantity 2700 in a MILLI file
        self.assertAlmostEqual(wall.bottom, 3.0)
        self.assertAlmostEqual(wall.top, 5.7)

    def test_a_wall_without_either_has_none(self):
        self.assertIsNone(wall.bottom); self.assertIsNone(wall.top)

    def test_a_foot_file_scales_storeys_and_walls(self):  # Review Focus 5
        # IfcSIUnit replaced by an IfcConversionBasedUnit FOOT; storey at 10 ft, Height 9 ft
        self.assertAlmostEqual(model.storeys[0].elevation, 3.048, places=3)
        self.assertAlmostEqual(wall.top - wall.bottom, 2.7432, places=3)

@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WindowFallbackTest(unittest.TestCase):
    def test_overall_size_gives_the_area(self):
        # MILLI file, IfcWindow OverallHeight=1200, OverallWidth=1000, no quantities
        self.assertAlmostEqual(generic.area, 1.2)

    def test_geometry_gives_the_area_when_nothing_else_does(self):
        # METRE file, window with a 1.0 x 1.5 x 0.05 box representation, no sizes
        self.assertAlmostEqual(generic.area, 1.5, places=1)
```

Extend `SampleFileTest` with:

```python
    def test_the_sample_house_has_a_storey_and_a_roof(self):
        self.assertEqual([s.name for s in model.storeys], ['00 groundfloor'])
        self.assertAlmostEqual(model.storeys[0].floor_area, 25.8, places=1)
        self.assertAlmostEqual(sum(s.area for s in model.slabs if s.kind == 'roof'), 53.6, places=1)
        left = next(w for w in model.walls if w.name == 'house - outer wall - house left')
        self.assertAlmostEqual(left.bottom, -0.25, places=2)
        self.assertAlmostEqual(left.top, 3.38, places=2)
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -3`
Expected: FAIL with `AttributeError: 'GenericWall' object has no attribute 'bottom'` and the window assertions at `0.0`.

- [ ] **Step 3: Implement**

`get_wall_elevations` as in Interfaces; set `bottom`/`top` in both wall extractors. In `extract_ifc_window_to_generic`, after the quantities block and before the existing height × width fallback, read `getattr(window, 'OverallHeight', None)` / `OverallWidth` into `height`/`width` when the quantities gave none; after that, if `area` is still 0, use `get_side_area_from_geometry(window)` (skip when it returns None). In `analyze_ifc.py`, the summary becomes `"Extracted {} walls, {} windows, {} slabs and {} storeys"`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -3`
Expected: `OK`.

- [ ] **Step 5: Regenerate the sample extractions**

```bash
.venv/bin/python pyrevit-extension/Makit.extension/lib/analyze_ifc.py examples/IFC/Building-Architecture.ifc --extract-only --output examples/IFC/building-model.json
.venv/bin/python pyrevit-extension/Makit.extension/lib/analyze_ifc.py examples/IFC/Ifc4_ARC.ifc --extract-only --output examples/IFC/Ifc4_ARC.json
.venv/bin/python pyrevit-extension/Makit.extension/lib/analyze_ifc.py examples/IFC/Ifc2x3_Duplex_Architecture.ifc --extract-only --output examples/IFC/Ifc2x3_Duplex_Architecture.json
```

Check the figures against the spec table with a one-off `python -c` over each JSON: Σ `storeys[].floorArea` = 25.8 / 299.3 / 718.8 (±0.2); Σ roof `slabs[].area` = 53.6 / 133–137 / 244.3; Duplex Σ window area > 0. Then `cargo test -p makit-tools` must still pass (the existing `test_load_building_model` and `test_analyze_wwr_sample` read `building-model.json`).

- [ ] **Step 6: Commit**

```bash
git add pyrevit-extension/ examples/IFC/building-model.json examples/IFC/Ifc4_ARC.json examples/IFC/Ifc2x3_Duplex_Architecture.json
git commit -m "Record wall elevations and measure sizeless windows"
```

---

### Task 3: The envelope in Rust

**Files:**
- Modify: `crates/makit-tools/src/building_model.rs` (types at lines 14–105; new functions after `window_area_by_facing` at line 245; tests after line 378)

**Interfaces:**
- Produces:

```rust
pub struct Storey { pub id: String, pub name: String, pub elevation: f64,
                    #[serde(rename = "floorArea")] pub floor_area: f64,
                    #[serde(rename = "slabIds")] pub slab_ids: Vec<String> }   // all #[serde(default)]
#[serde(rename_all = "lowercase")]
pub enum SlabKind { #[default] Floor, Roof, Landing, Other }
pub struct Slab { pub id: String, pub name: String, pub kind: SlabKind, pub area: f64,
                  pub top: Option<f64>, pub bottom: Option<f64>, pub storey: Option<String>,
                  #[serde(rename = "type")] pub slab_type: String,
                  #[serde(rename = "isExternal")] pub is_external: Option<bool>,
                  pub properties: serde_json::Value }
// BuildingModel: pub storeys: Vec<Storey>, pub slabs: Vec<Slab>
// GenericWall:   pub bottom: Option<f64>, pub top: Option<f64>
// ModelMetadata: #[serde(default = "format_one", rename = "formatVersion")] pub format_version: u32
impl BuildingModel { pub fn format_version(&self) -> u32 }   // 1 when metadata is None

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoreyArea { pub name: String, pub elevation: f64, pub area: f64, pub plates: usize }
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Envelope {
    pub grade: f64, pub gfa: f64, pub storeys: Vec<StoreyArea>, pub empty_storeys: usize,
    pub walls_ag: f64, pub walls_bg: f64, pub walls_without_height: usize,
    pub windows: f64, pub windows_by_facing: Vec<(f64, f64)>,
    pub roof: f64, pub roofs: Vec<(String, f64)>,
    pub left_out: Vec<(String, f64)>,   // floor plates with no storey
    pub format_version: u32,
}
pub fn below_grade_fraction(bottom: f64, top: f64, grade: f64) -> f64  // in [0, 1]; 0 when top <= bottom
pub fn envelope(model: &BuildingModel, grade: f64) -> Envelope
pub fn print_envelope(env: &Envelope)   // println! table; used by the CLI and murb handler
```

- Envelope walls follow `analyze_wwr`'s envelope: every wall with `is_external != Some(false)`; `windows` = windows in those walls + model windows listed in no wall (the `loose` logic in `analyze_wwr`, extracted into a shared helper `fn envelope_windows(model) -> (Vec<&WallWindow>, Vec<&GenericWindow>)`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn below_grade_fraction_clamps_and_handles_degenerate_walls() {
    assert_eq!(below_grade_fraction(-1.5, 2.0, 0.0), 1.5 / 3.5);
    assert_eq!(below_grade_fraction(0.0, 3.0, 0.0), 0.0);
    assert_eq!(below_grade_fraction(-2.0, -1.0, 0.0), 1.0);
    assert_eq!(below_grade_fraction(1.0, 1.0, 0.0), 0.0);   // Review Focus 2
    assert_eq!(below_grade_fraction(2.0, 1.0, 0.0), 0.0);
    assert_eq!(below_grade_fraction(-1.0, 1.0, 0.5), 0.75);
}

#[test]
fn envelope_sums_storeys_roofs_and_splits_walls_at_grade() {
    // model: storeys L1 (0.0, floorArea 100, 2 plates) and L2 (3.0, 80, 1 plate), one empty storey;
    // slabs: roof "r" 60.0, floor "pad" with storey null area 50;
    // walls: "a" ext area 30 bottom -1.5 top 2.0 windows 4.0; "b" (no flag) area 20 no elevations;
    //        "in" interior area 9 windows 1.0; model.windows: skylight 2.0 (in no wall)
    let env = envelope(&model, 0.0);
    assert_eq!(env.gfa, 180.0);
    assert_eq!(env.storeys.len(), 3);
    assert_eq!(env.empty_storeys, 1);
    assert_eq!(round1(env.walls_bg), round1(30.0 * 1.5 / 3.5));
    assert_eq!(round1(env.walls_ag), round1(30.0 * 2.0 / 3.5 + 20.0));
    assert_eq!(env.walls_without_height, 1);
    assert_eq!(env.windows, 6.0);
    assert_eq!(env.roof, 60.0);
    assert_eq!(env.left_out, vec![("pad".to_string(), 50.0)]);
}

#[test]
fn a_format_one_model_has_no_envelope_areas() {
    // the existing write_model-style JSON without storeys/slabs/metadata
    let env = envelope(&model, 0.0);
    assert_eq!((env.gfa, env.roof, env.format_version), (0.0, 0.0, 1));
}

#[test]
fn the_sample_extractions_match_the_probed_figures() {
    // skip when the files are missing, like test_load_building_model
    let house = envelope(&load_model(Path::new("../../examples/IFC/building-model.json")).unwrap(), 0.0);
    assert_eq!(round1(house.gfa), 25.8);
    assert_eq!(round1(house.roof), 53.6);
    assert_eq!(round1(house.walls_ag + house.walls_bg), 36.4);
    assert!(house.walls_bg > 2.5 && house.walls_bg < 3.5);
    let duplex = envelope(&load_model(Path::new("../../examples/IFC/Ifc2x3_Duplex_Architecture.json")).unwrap(), 0.0);
    assert_eq!(round1(duplex.gfa), 299.3);
    assert!(duplex.windows > 0.0);
    let arc = envelope(&load_model(Path::new("../../examples/IFC/Ifc4_ARC.json")).unwrap(), 0.0);
    assert_eq!(round1(arc.roof), 244.3);
    assert_eq!(arc.left_out.len(), 10);   // Pad:Pad 2 and nine pile stubs
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p makit-tools building_model 2>&1 | tail -20`
Expected: compile errors, `cannot find function envelope`.

- [ ] **Step 3: Implement the types and functions**

As in Interfaces. `print_envelope` prints, in order: `GFA` with each storey as `    Level 1    0.00 m   180.5 m²  (10 plates)`, then `empty_storeys` and `left_out` lines when non-zero, `Walls above grade` / `below grade (grade 0.0 m)` with the `walls_without_height` note, `Windows` with the facing split as `68% 143°, …`, `Roof` with each roof. Round to one decimal when printing only.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p makit-tools 2>&1 | tail -5`
Expected: all pass, including the pre-existing sample tests.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/building_model.rs
git commit -m "Derive the MURB envelope from a building model"
```

---

### Task 4: Python discovery shared by the IFC extractor and MURB

**Files:**
- Create: `crates/makit-tools/src/python.rs`
- Modify: `crates/makit-tools/src/lib.rs` (add `pub mod python;`)
- Modify: `crates/makit-tools/src/murb.rs:282-325` (delete `find_runner_script`, `find_python`; call sites at 328–329 and 510)

**Interfaces:**
- Produces:

```rust
pub const SETUP_HINT: &str = "set up a Python with IfcOpenShell once: uv venv .venv --python 3.13 && uv pip install --python .venv/bin/python ifcopenshell (or point MAKIT_PYTHON at one)";
/// A candidate interpreter and whether the user named it explicitly (MAKIT_PYTHON).
pub struct Candidate { pub path: PathBuf, pub explicit: bool }
pub fn candidates() -> Vec<Candidate>          // MAKIT_PYTHON, <repo>/.venv/bin/python (Scripts\python.exe on Windows), python3, python
pub fn find(module: &str) -> anyhow::Result<PathBuf>   // = find_with(candidates(), module, &imports)
pub fn find_with(candidates: Vec<Candidate>, module: &str, probe: &dyn Fn(&Path, &str) -> bool) -> anyhow::Result<PathBuf>
pub fn imports(python: &Path, module: &str) -> bool   // `python -c "import <module>"` succeeds
pub fn repo_root() -> anyhow::Result<PathBuf>     // walk up from current_exe's dir, then from cwd, to a dir containing scripts/murb_runner.py
pub fn repo_file(relative: &str) -> anyhow::Result<PathBuf>
```

- `find_with`: an explicit candidate that fails the probe is the error (`"MAKIT_PYTHON={} cannot import {module}"`), no fallback; otherwise the first passing candidate; none → error listing the paths tried, the module, and `SETUP_HINT`.
- `murb.rs` uses `python::find("murb_energy_tool")` in `run_simulation` and `handle_connect`, and `python::repo_file("scripts/murb_runner.py")`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_explicit_python_without_the_module_is_an_error_not_a_fallback() {
    let cands = vec![cand("/x/py", true), cand("/usr/bin/python3", false)];
    let err = find_with(cands, "ifcopenshell", &|p, _| p.ends_with("python3")).unwrap_err().to_string();
    assert!(err.contains("MAKIT_PYTHON=/x/py cannot import ifcopenshell"), "{err}");
}

#[test]
fn the_first_candidate_that_imports_the_module_wins() {
    let cands = vec![cand(".venv/bin/python", false), cand("python3", false)];
    assert_eq!(find_with(cands, "ifcopenshell", &|p, _| p.ends_with("python3")).unwrap(), PathBuf::from("python3"));
}

#[test]
fn no_candidate_names_what_was_tried_and_how_to_set_up() {
    let err = find_with(vec![cand("python3", false)], "ifcopenshell", &|_, _| false).unwrap_err().to_string();
    assert!(err.contains("python3") && err.contains("ifcopenshell") && err.contains("uv venv"), "{err}");
}

#[test]
fn repo_file_finds_the_runner_from_the_test_cwd() {
    assert!(repo_file("scripts/murb_runner.py").unwrap().ends_with("scripts/murb_runner.py"));
    assert!(repo_file("pyrevit-extension/Makit.extension/lib/analyze_ifc.py").unwrap().exists());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p makit-tools python 2>&1 | tail -10`
Expected: compile error, `unresolved import crate::python`.

- [ ] **Step 3: Implement `python.rs` and switch `murb.rs` to it**

`candidates()` reads `MAKIT_PYTHON` with `std::env::var`, then `repo_root().ok().map(|r| r.join(".venv").join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" }))` when it exists, then `python3`, `python`. `imports` runs `Command::new(python).args(["-c", &format!("import {module}")])` with stdout/stderr null. Delete `find_runner_script`/`find_python` from `murb.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p makit-tools 2>&1 | tail -5`
Expected: all pass (`murb_simulate_sends_the_window_directions` and friends untouched).

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/python.rs crates/makit-tools/src/lib.rs crates/makit-tools/src/murb.rs
git commit -m "Find a Python for the bridges in one place"
```

---

### Task 5: Extraction on demand and the `ifc` source

**Files:**
- Modify: `crates/makit-tools/src/ifc.rs` (replace the 15-line placeholder)
- Modify: `crates/makit-tools/src/building_model.rs:178-186` (`extraction_command` now prints `makit extract <ifc>` instead of the Python command; keep the function, used in error hints)

**Interfaces:**
- Consumes: `python::find`, `python::repo_file`, `building_model::{find_json_for_ifc, extraction_path, load_model}`, `makit_core::models::CancelToken`.
- Produces:

```rust
pub const EXTRACTOR: &str = "pyrevit-extension/Makit.extension/lib/analyze_ifc.py";
pub const STALE_TOLERANCE: std::time::Duration = Duration::from_secs(2);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved { pub json: PathBuf, pub extracted: bool, pub note: Option<String> }
pub fn extract(ifc: &Path, output: Option<&Path>, cancel: Option<&CancelToken>) -> anyhow::Result<PathBuf>
pub fn is_stale(ifc: &Path, json: &Path) -> bool
pub fn needs_extraction(ifc: &Path) -> bool            // no JSON, or stale
pub fn resolve(ifc: &Path, found: Option<PathBuf>, stale: bool, extract: impl FnOnce() -> anyhow::Result<PathBuf>) -> anyhow::Result<Resolved>
pub fn extraction_for(ifc: &Path, cancel: Option<&CancelToken>) -> anyhow::Result<Resolved>
pub fn register_tasks(reg: &mut Registry)              // `ifc` source: --file (required), --output, --force (bool)
```

- `extract` writes to `<output>.tmp` (`output` defaults to `extraction_path(ifc)`), runs `python <EXTRACTOR> <ifc> --extract-only --output <tmp>` with stdout inherited and stderr piped, polls `try_wait` every 100 ms while checking `cancel`; on success renames tmp over `output`; on failure or cancel removes tmp and returns the stderr (or "cancelled").
- `resolve` decision table: `found` fresh → `Resolved { json, extracted: false, note: None }`; `found` stale → extract; `Ok(p)` → `{ p, true, None }`, `Err(e)` → `{ found, false, Some("using stale <json>: <e>") }`; missing → extract; `Ok(p)` → `{ p, true, None }`, `Err(e)` → `Err("<ifc> is not extracted and extraction failed: <e>")`.
- `is_stale`: mtime difference beyond `STALE_TOLERANCE`, or the JSON's `formatVersion` < 2 (read via `load_model(json).map(|m| m.format_version()).unwrap_or(1)`).
- The source handler: `force` → `extract(file, output, Some(&ctx.cancel))`; else `extraction_for`; prints the JSON path, `extracted`/`fresh`/note, and a one-line summary (`walls, windows, slabs, storeys` counts from `load_model`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_fresh_extraction_is_used_without_extracting() {
    let r = resolve(Path::new("a.ifc"), Some("a.json".into()), false, || panic!("must not extract")).unwrap();
    assert_eq!(r, Resolved { json: "a.json".into(), extracted: false, note: None });
}

#[test]
fn a_stale_extraction_is_replaced_or_kept_with_a_note() {           // Review Focus 3
    let r = resolve(Path::new("a.ifc"), Some("a.json".into()), true, || Ok("a.json".into())).unwrap();
    assert!(r.extracted && r.note.is_none());
    let r = resolve(Path::new("a.ifc"), Some("a.json".into()), true, || Err(anyhow!("no python"))).unwrap();
    assert_eq!(r.json, PathBuf::from("a.json"));
    assert!(!r.extracted);
    assert!(r.note.as_deref().unwrap().contains("no python"));
}

#[test]
fn a_missing_extraction_that_cannot_be_made_is_an_error() {
    let err = resolve(Path::new("a.ifc"), None, false, || Err(anyhow!("no python"))).unwrap_err().to_string();
    assert!(err.contains("a.ifc") && err.contains("no python"), "{err}");
}

#[test]
fn staleness_tolerates_a_checkout_and_old_formats() {
    // in a temp folder: write json, then ifc 1 s later → not stale; set the ifc 5 s newer (filetime via File::set_modified) → stale;
    // a format-1 json (no metadata.formatVersion) with an older ifc → stale
}

#[test]
fn a_bare_filename_resolves_beside_itself() {                       // Review Focus 4
    assert_eq!(building_model::extraction_path(Path::new("lonely.ifc")), PathBuf::from("lonely.json"));
    // in a temp folder: an ifc with a fresh json beside it needs no extraction; alone it does
    let dir = folder("bare"); let ifc = dir.join("lonely.ifc");
    touch(&ifc);
    assert!(needs_extraction(&ifc));
    write_format2_model(&dir.join("lonely.json"));          // written after the ifc, so newer
    assert!(!needs_extraction(&ifc));
    let r = extraction_for(&ifc, None).unwrap();            // must not look for Python
    assert_eq!(r, Resolved { json: dir.join("lonely.json"), extracted: false, note: None });
}

#[test]
fn a_failed_extraction_leaves_no_temp_file() {
    // MAKIT_PYTHON cannot be set safely in tests: call extract with a nonexistent ifc path so the extractor exits non-zero
    // (needs a Python with ifcopenshell; skip with eprintln when python::find("ifcopenshell") fails)
    let dir = folder("tmpfile"); let ifc = dir.join("missing.ifc");
    assert!(extract(&ifc, None, None).is_err());
    assert!(!dir.join("missing.json.tmp").exists() && !dir.join("missing.json").exists());
}

#[test]
fn the_ifc_source_registers_its_options() {
    let mut reg = Registry::default(); register_tasks(&mut reg);
    let names: Vec<_> = reg.sources["ifc"].options.iter().map(|o| (o.name.as_str(), o.required)).collect();
    assert_eq!(names, vec![("file", true), ("output", false), ("force", false)]);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p makit-tools ifc:: 2>&1 | tail -10`
Expected: compile errors for the missing functions.

- [ ] **Step 3: Implement `ifc.rs`**

As in Interfaces. Use `std::fs::File::set_modified` (stable since 1.75) in the staleness test. Keep `register_tasks` exporting the same `ifc` source name so `node.rs`'s picker binding (`Source("ifc")` → `file`) keeps working.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p makit-tools 2>&1 | tail -5`
Expected: all pass. Then by hand: `cargo run -q -p makit -- list | grep -A3 "◆ ifc"` shows the three options.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/ifc.rs crates/makit-tools/src/building_model.rs
git commit -m "Extract IFC files on demand through the ifc source"
```

---

### Task 6: `murb-simulate` fills its envelope from the model

**Files:**
- Modify: `crates/makit-tools/src/murb.rs` (`window_groups_from_model` 211–250, `window_groups` 252–267, `simulation_input` 419–448, `handle_simulate` 450–480, `register_tasks` 531–637, tests from 639)

**Interfaces:**
- Consumes: `building_model::{envelope, Envelope, print_envelope, load_model}`, `ifc::extraction_for`.
- Produces:

```rust
/// The model named by `--model`, loaded once: its envelope at `grade` and the JSON it came from.
fn model_envelope(ctx: &TaskContext, grade: f64) -> anyhow::Result<Option<(Envelope, PathBuf)>>
/// One envelope quantity: typed wins, else the model, else an error naming the option.
fn resolve_area(ctx: &TaskContext, option: &str, model: Option<&(Envelope, PathBuf)>, pick: fn(&Envelope) -> f64, zero_is_gap: bool) -> anyhow::Result<(f64, String)>
fn simulation_input(ctx: &TaskContext) -> anyhow::Result<(MurbInput, Vec<String>)>   // the Vec is one source line per quantity, window split last
pub fn window_groups_from_model(path: &Path) -> anyhow::Result<(Vec<WindowGroupInput>, PathBuf, f64)>  // unchanged signature; no windows → default_window_groups() and area 0.0
```

- Error texts: `"{option}: give --{option} or --model"` without a model; `"{option}: the model {json} has no {what}; give --{option}"` on a zero gap where `what` is `storeys` (gfa), `above-grade walls`, `roof`; for `format_version < 2`: `"{option}: {json} predates storeys and slabs; re-extract it with makit extract"`.
- Registry: `gfa`, `walls-ag`, `windows`, `roof` → `required: false`; `walls-bg` default `None`; new `grade` option (`"float"`, `false`, `Some("0")`) right after `model`; `model` description `"IFC file or extraction: envelope areas and window directions come from it"`.
- `handle_simulate` prints `print_envelope` when a model was used, then the per-quantity source lines, before running.

- [ ] **Step 1: Write the failing tests**

Extend the existing `write_model` helper with a `with_envelope: bool` variant writing storeys `[{name: "L1", elevation: 0, floorArea: 120, slabIds: ["p"]}]`, slabs `[{id: "p", kind: "floor", area: 120, storey: "L1"}, {id: "r", kind: "roof", area: 90}]`, walls `e` (bottom −1.0, top 3.0, area 10) and `s` (bottom 0, top 3, area 10), metadata `formatVersion: 2`.

```rust
#[test]
fn the_model_fills_every_untyped_area() {
    let (input, sources) = simulation_input(&context_with_model(&[])).unwrap();   // epw + model only
    assert_eq!((input.gfa, input.area_roof, input.area_windows), (120.0, 90.0, 4.0));
    assert_eq!(round1(input.area_walls_bg), 2.5);        // 10 * 1/4
    assert_eq!(round1(input.area_walls_ag), 17.5);
    assert!(sources.iter().any(|s| s.starts_with("GFA") && s.contains("house.json")), "{sources:?}");
}

#[test]
fn typed_areas_win_over_the_model() {
    let (input, _) = simulation_input(&context_with_model(&[("gfa", "500"), ("walls-bg", "7")])).unwrap();
    assert_eq!((input.gfa, input.area_walls_bg), (500.0, 7.0));
    assert_eq!(input.area_roof, 90.0);
}

#[test]
fn grade_moves_the_split() {
    let (input, _) = simulation_input(&context_with_model(&[("grade", "3")])).unwrap();
    assert_eq!(round1(input.area_walls_bg), 20.0);
}

#[test]
fn without_a_model_missing_areas_name_their_option() {
    let err = simulation_input(&context(&[])).unwrap_err().to_string();   // epw only
    assert!(err.contains("gfa: give --gfa or --model"), "{err}");
}

#[test]
fn a_model_zero_is_a_gap_for_gfa_walls_and_roof_but_not_windows() {       // Review Focus 1
    // model without storeys and without roof but with windowless walls
    let err = simulation_input(&ctx).unwrap_err().to_string();
    assert!(err.contains("gfa: the model") && err.contains("no storeys"), "{err}");
    // with --gfa and --roof typed, windows = 0 is accepted and the split is even
    let (input, _) = simulation_input(&ctx_with(&[("gfa", "100"), ("roof", "40")])).unwrap();
    assert_eq!(input.area_windows, 0.0);
    assert_eq!(input.window_groups.len(), 4);
}

#[test]
fn a_format_one_extraction_says_to_re_extract() {
    let err = simulation_input(&context(&[("model", old_json)])).unwrap_err().to_string();
    assert!(err.contains("predates storeys") && err.contains("makit extract"), "{err}");
}

#[test]
fn a_json_model_never_needs_python() {                                   // Review Focus 4
    // the model path is a .json: simulation_input succeeds with MAKIT_PYTHON unset and no .venv in the temp dir
}
```

Update `murb_simulate_sends_the_window_directions` so `gfa`, `walls-ag`, `windows`, `roof`, `grade` are asserted `!required` and `walls-bg` has `default: None`; update `a_model_that_cannot_give_directions_is_an_error` (no windows is no longer an error for directions; the `not extracted` case now comes from `ifc::extraction_for` and reads `"is not extracted and extraction failed"` when no Python can be found — make that assertion tolerant of either an extraction attempt or the failure message).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p makit-tools murb 2>&1 | tail -15`
Expected: compile errors (`context_with_model`, tuple shape of `simulation_input`) and failing assertions.

- [ ] **Step 3: Implement**

As in Interfaces. `model_envelope` resolves `.ifc` through `ifc::extraction_for(path, Some(&ctx.cancel))` and `.json` through `load_model`; `window_groups` takes its groups from the same loaded model (load once). Keep `parse_window_directions` untouched.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p makit-tools 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/murb.rs
git commit -m "Take murb-simulate's envelope from the model"
```

---

### Task 7: CLI: `makit extract`, auto-extracting `analyze`, `.ifc` for `wall-orientation-wwr`

**Files:**
- Create: `crates/makit-cli/src/commands/extract.rs`
- Modify: `crates/makit-cli/src/commands/mod.rs`, `crates/makit-cli/src/main.rs:14-48, 65-90`, `crates/makit-cli/src/commands/analyze.rs`
- Modify: `crates/makit-tools/src/analysis.rs:16-26`

**Interfaces:**
- Consumes: `ifc::{extract, extraction_for}`, `building_model::{envelope, print_envelope, analyze_wwr, print_wwr_report, load_model}`.
- Produces: `commands::extract::run(file: &str, output: Option<&str>, force: bool) -> Result<()>`; `commands::analyze::run(file: &str, analysis_type: &str, grade: f64) -> Result<()>`; clap `Commands::Extract { file, #[arg(short, long)] output: Option<String>, #[arg(long)] force: bool }` and `Analyze { …, #[arg(long, default_value_t = 0.0)] grade: f64 }`.
- `analyze`: an `.ifc` goes through `extraction_for(path, None)`; print `"  Extracted to {json}"` when `extracted`, the note when present, `"  Found extracted model: {json}"` otherwise. Types: `summary` (default: metadata, WWR report, envelope), `wwr`/`wall-orientation`/`orientation` (WWR only), `envelope` (envelope only).
- `wall-orientation-wwr`: when `--input` ends in `.ifc`, resolve it with `extraction_for(path, Some(&ctx.cancel))` and print the same found/extracted line.

- [ ] **Step 1: Implement** (the CLI crate has no unit tests; verification is by running)

- [ ] **Step 2: Verify by running**

```bash
cargo run -q -p makit -- extract examples/IFC/Building-Architecture.ifc --output /tmp/makit-house.json
cargo run -q -p makit -- analyze examples/IFC/Ifc2x3_Duplex_Architecture.ifc
cargo run -q -p makit -- analyze examples/IFC/Ifc4_ARC.ifc -t envelope --grade -1.5
cargo run -q -p makit -- exec analysis analysis wall-orientation-wwr -o input=examples/IFC/Building-Architecture.ifc
MAKIT_PYTHON=/usr/bin/python3 cargo run -q -p makit -- analyze examples/IFC/Building-Architecture.ifc
```

Expected: the first writes the JSON and prints the summary; the second prints "Found extracted model" (committed, fresh) then WWR and an envelope with GFA 299.3; the third shows walls moving to above grade at −1.5 m; the fourth prints the WWR table from the committed JSON; the fifth prints `MAKIT_PYTHON=/usr/bin/python3 cannot import ifcopenshell` only if a stale/missing extraction forces Python, otherwise uses the JSON.

- [ ] **Step 3: `cargo fmt && cargo clippy --all-targets` clean, `cargo test` passes. Commit**

```bash
git add crates/makit-cli crates/makit-tools/src/analysis.rs
git commit -m "Add makit extract and extract on demand in analyze"
```

---

### Task 8: TUI: extraction in the picker, envelope line, reload after `ifc`

**Files:**
- Modify: `crates/makit-tui/src/msg.rs:20-31, 94-107` (IfcOutcome, Msg)
- Modify: `crates/makit-tui/src/app.rs:35-43, 150-170` (RunningTask, IfcState)
- Modify: `crates/makit-tui/src/update.rs:141, 388-413, 453-470, 481-509` (dispatch, run_or_prompt, task_finished, ifc_loaded)
- Modify: `crates/makit-tui/src/runtime.rs:242-292` (spawn_ifc_load, load_ifc)
- Modify: `crates/makit-tui/src/view.rs:377-490` (render_ifc_details)
- Tests: in each module's `mod tests`; view tests at 1280–1312; update tests at 830–902; runtime tests at ~350–370

**Interfaces:**
- Consumes: `ifc::{needs_extraction, extraction_for}`, `building_model::{envelope, Envelope}`.
- Produces:

```rust
// msg.rs
pub enum IfcOutcome {
    Analysis { analysis: WwrAnalysis, envelope: Envelope, json: String, extracted: bool, note: Option<String> },
    NeedsExtraction { reason: String },
}
Msg::IfcExtracting { path: String }
// app.rs
IfcState { …, pub envelope: Option<Envelope>, pub note: Option<String> }
RunningTask { …, pub reload_ifc: Option<String> }   // set in run_or_prompt for NodeId::Source("ifc") from options["file"]
// runtime.rs
pub fn load_ifc(path: &Path, on_extract: &dyn Fn()) -> Result<IfcOutcome, String>
```

- `update`: `IfcExtracting { path }` → status `"◌ Extracting {name}… (Python)"` when `state.ifc.loading == Some(path)`. `ifc_loaded`: `Analysis` sets `envelope`, `note`, status `"◆ {walls} walls, WWR {wwr:.1}% — {path}"` plus `" (extracted to {json name})"` when `extracted`, plus `" · {note}"` when noted; `NeedsExtraction { reason }` sets `note = Some(reason)` and status `"◆ {path} is not extracted: {reason}"`. `task_finished`: on `Ok` and not stopped, when `task.reload_ifc == state.ifc.loaded` push `Cmd::LoadIfc { path }` and set `state.ifc.loading`.
- `view`: with an analysis, one more line after the WWR summary: `"GFA {gfa:.0} m² · walls {ag:.0} / {bg:.0} m² · windows {w:.0} m² · roof {r:.0} m²"`, and the note dimmed below it when present; without analysis or error, the second line is `state.ifc.note` or `"Not extracted yet: no {json} beside it"`.
- `runtime::load_ifc`: for `.ifc`, if `ifc::needs_extraction(path)` call `on_extract()`, then `ifc::extraction_for(path, None)`; map `Err` to `NeedsExtraction { reason }` when `find_json_for_ifc` is `None`, else to `Err(String)`; compute `envelope(&model, 0.0)` next to `analyze_wwr`. `spawn_ifc_load` passes a closure sending `Msg::IfcExtracting`.

- [ ] **Step 1: Write the failing tests**

update.rs:

```rust
#[test]
fn extracting_shows_while_the_selection_is_loading() {
    let mut s = state(); s.ifc.loading = Some("examples/IFC/b.ifc".into());
    update(&mut s, Msg::IfcExtracting { path: "examples/IFC/b.ifc".into() });
    assert!(s.status.starts_with("◌ Extracting b.ifc"));
    update(&mut s, Msg::IfcExtracting { path: "examples/IFC/a.ifc".into() });   // stale: ignored
    assert!(s.status.starts_with("◌ Extracting b.ifc"));
}

#[test]
fn an_extracted_result_says_so_and_keeps_the_envelope() {
    // IfcLoaded Analysis { extracted: true, note: None, envelope: sample_envelope() }
    assert!(s.status.contains("extracted to building-model.json"));
    assert!(s.ifc.envelope.is_some());
}

#[test]
fn a_missing_extraction_explains_why() {
    // IfcLoaded NeedsExtraction { reason: "no Python with ifcopenshell".into() }
    assert_eq!(s.status, "◆ m/y.ifc is not extracted: no Python with ifcopenshell");
    assert_eq!(s.ifc.note.as_deref(), Some("no Python with ifcopenshell"));
}

#[test]
fn finishing_the_ifc_source_reloads_the_picked_file() {
    // RunningTask for Source("ifc") with reload_ifc = Some(loaded path); TaskFinished Ok → [Cmd::LoadIfc { path }]
}
```

Update `ifc_picker_loads_on_enter_and_ignores_stale_results` and `ifc_errors_and_missing_extraction_clear_analysis` to the new variants.

view.rs: `an_ifc_without_extraction_says_what_is_missing` keeps its assertion when `note` is None; add `the_detail_shows_the_envelope_line` asserting a rendered line contains `"GFA 26 m²"` for `sample_envelope()`; add `a_noted_extraction_shows_the_note`.

runtime.rs: the existing `load_ifc` test on `Building-Architecture.ifc` passes `&|| {}` and asserts `extracted == false` and `envelope.gfa` rounds to 25.8. The existing `lonely.ifc` temp test (a file holding only `ISO-10303-21;`, no JSON beside it) now asserts: `on_extract` was called once (count with a `Cell<u32>`), and the result is `Ok(NeedsExtraction { reason })` with `reason` non-empty, whether Python was found (the extractor rejects the bogus file) or not (no interpreter). Add `a_json_selection_never_extracts`: `load_ifc` on `examples/IFC/building-model.json` never calls `on_extract` and returns `Analysis { extracted: false, .. }`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p makit-tui 2>&1 | tail -15`
Expected: compile errors on the new variants and fields.

- [ ] **Step 3: Implement**

As in Interfaces. Keep `layout::compute` untouched; the envelope line takes the row after the summary only when `y < bottom`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p makit-tui 2>&1 | tail -5`
Expected: all pass. Then by hand: `cargo run -p makit -- tui`, pick `Ifc4_ARC.ifc` (fresh JSON: instant), `touch examples/IFC/Ifc4_ARC.ifc` and pick again (status shows Extracting, then "extracted to"), expand `murb-simulate`, type `epw`, press `e`, see the envelope lines in Output.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tui
git commit -m "Extract picked IFC files in the TUI and show the envelope"
```

---

### Task 9: Docs, counts and end-to-end verification

**Files:**
- Modify: `README.md:29, 66, 70-102, 184-209, 234-262`; `AGENTS.md:39, 42-46, 60-64, 93, 102`; `CLAUDE.md:14-27, 46`; `docs/IFC_SUPPORT.md`; `examples/IFC/README.md`

**Interfaces:** none new. Documents what Tasks 1–8 built, with the exact rules from Global Constraints.

- [ ] **Step 1: README**

Features row for IFC Analysis mentions storeys, roofs and automatic extraction; MURB row says the envelope comes from the model. CLI block: replace the hand-run extractor line with `makit extract model.ifc` and show the one-line `murb-simulate -o model=model.ifc -o epw=weather.epw`; keep the typed-areas example as "override anything". Module Status: `ifc` source → Working ("Extracts an IFC to JSON beside it with the Python extractor; run by the picker, `makit analyze`, `makit extract` and `murb-simulate` when the JSON is missing or stale"); `wall-orientation-wwr` → Working; `murb-simulate` note gains "envelope areas (GFA by storey, walls above and below grade, windows, roof) from the model; typed values win". Development: `MAKIT_PYTHON`, and that `cargo test` reads the three committed extractions. Update the test counts from the final `cargo test` and Python runs.

- [ ] **Step 2: AGENTS.md, CLAUDE.md**

Add `extract` to the CLI subcommand lists and `python.rs` to the module list; `cargo test` counts; the IFC trap-table row gains: "floor plates attach to the storey within 0.3 m of their top face and a storey's area is the footprint union of its plates; roofs count up-facing faces (normal z > 0.5); a zero from the model is an error for gfa/walls-ag/roof, a value for windows/walls-bg; grade defaults to project z = 0 (`--grade`)". The MURB row: `--model` now fills every envelope area; `walls-bg` has no registered default on purpose.

- [ ] **Step 3: IFC_SUPPORT.md and examples/IFC/README.md**

Document the new JSON keys (`storeys`, `slabs`, wall `bottom`/`top`, `metadata.formatVersion`), the plate/roof rules, `makit extract`, staleness, and that the two extra sample extractions are committed while their IFC files are not.

- [ ] **Step 4: End-to-end run**

```bash
uv pip install --python .venv/bin/python numpy scipy pandas pvlib tabulate   # murb's needs, for verification only
PYTHONPATH=../murb MAKIT_PYTHON=.venv/bin/python cargo run -q -p makit -- exec murb analysis murb-simulate \
  -o model=examples/IFC/Ifc2x3_Duplex_Architecture.ifc \
  -o epw="examples/EPW/Toronto_TMU/CAN_ON_Toronto.City-Univ.of.Toronto.715080_TMYx.epw" -o output=/tmp/duplex.json
```

Repeat for `Building-Architecture.ifc` and `Ifc4_ARC.ifc`. Expected: each prints the envelope breakdown and a TEDI/TEUI/GHGI report; the Duplex GFA line reads 299.3 m² over Level 1 and Level 2. Record the three outputs' header lines for the PR description.

- [ ] **Step 5: Final checks and commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test 2>&1 | grep "test result"
.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -2
python3 -m unittest discover -s scripts/tests 2>&1 | tail -2
git status --short   # only intended files; examples/EPW stays untracked
git add README.md AGENTS.md CLAUDE.md docs/IFC_SUPPORT.md examples/IFC/README.md
git commit -m "Document IFC extraction on demand and the MURB envelope"
```
