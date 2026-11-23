# IFC Isometric Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The TUI's Detail pane draws the IFC model loaded in the picker as a 30° isometric of its walls, from axes the extractor now writes in metres.

**Architecture:** The Python extractor (`pyrevit-extension/Makit.extension/lib/`) writes each wall's axis in metres and project coordinates, and every other wall and window length in metres, stamping format 3. The Rust model (`makit-tools`) reads the axes; `makit-geometry` gets a z-up isometric projection; the TUI keeps the loaded `BuildingModel` in its state and the `Building` preview draws it in two tones (exterior accent over interior dim) when the model has axes, falling back to today's rose and illustration otherwise.

**Tech Stack:** Rust (ratatui 0.29, serde), Python 3 with IfcOpenShell 0.9 (`.venv/bin/python`), `unittest`.

**Spec:** `docs/superpowers/specs/2026-11-23-ifc-isometric-preview-design.md`

## Global Constraints

- `cargo fmt && cargo clippy` clean before every commit; `cargo test` green at every commit.
- Python under `pyrevit-extension/` mirrors the existing style: snake_case, `.format()` strings, no f-strings or type hints; tests run with `.venv/bin/python -m unittest discover -s pyrevit-extension/tests`.
- Every length the extractor writes for walls and windows is in metres, project coordinates: `startPoint`, `endPoint`, `height`, `length`, `width`; window `height`, `width`, `position`. Areas keep following `--unit`.
- A wall axis is drawn only from a format 3 (or later) extraction; a degenerate axis is one whose ends are 1 mm apart or less.
- Isometric projection of project coordinates (x east, y north, z up), viewer at the south-west: `sx = (x − y)·cos 30°`, `sy = (x + y)·sin 30° + z`, screen y up.
- Preview wording: title `isometric` with a model, `isometric · illustration` without; captions `Walls {drawn} drawn` (plus ` · {skipped} without axis or height` when skipped > 0) and `{w:.1} × {d:.1} m footprint · {h:.1} m high`.
- Commit messages are short imperatives (`Add …`, `Fix …`) ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Commit only this work's files; `murb_results.*`, `results/` and `examples/EPW/` are the user's and stay untracked.

## Review Focus

- A model whose walls are all flagged interior: the main layer is empty, so the drawing's bounds must come from the faint layer or nothing is drawn (Task 7 test `an_all_interior_model_still_fills_the_pane`).
- A wall with an axis but no `bottom`/`top`: never drawn at height zero; counted as skipped (Task 7 test `walls_without_an_axis_or_height_are_counted_not_drawn`).
- An axis polyline with more than two points (a bent or segmented wall): first and last point, not a crash (Task 1 test `test_a_bent_axis_runs_from_its_first_to_its_last_point`).
- A format 3 extraction with no axes at all (the live Revit extractor's output): `has_axes()` false, the IFC node shows the rose (Task 4 test `has_axes_needs_format_three_and_one_axis`, Task 6 test `a_model_without_axes_keeps_the_rose`).
- A format 2 extraction that cannot be re-extracted (no Python): kept with a note, drawn as the rose, never from its stubs (Task 6 test `a_model_without_axes_keeps_the_rose` uses a format 2 model).

---

### Task 1: Extractor writes real wall axes in metres

**Files:**
- Modify: `pyrevit-extension/Makit.extension/lib/ifc_extractors.py` (`get_ifc_wall_geometry` at ~136–194, its call at ~339, `extract_ifc_curtain_wall_to_generic` at ~744–820)
- Test: `pyrevit-extension/tests/test_ifc_extractors.py`

**Interfaces:**
- Consumes: `get_faces(element)` → `[(normal, area, (a, b, c))]` in metres; `get_unit_scales(ifc_file)` → `(length_scale, area_scale)`; `ifcopenshell.util.placement.get_local_placement(placement)` → 4×4 ndarray.
- Produces:
  - `get_axis_points(wall)` → `[(x, y, z), (x, y, z)]` local coordinates (first and last point of an `Axis` `IfcPolyline` / `IfcIndexedPolyCurve`; 2D points get z 0) or `None`.
  - `to_project(matrix, point, length_scale)` → `(x, y, z)` metres.
  - `axis_from_faces(faces, direction, z=None)` → `((x0, y0, z), (x1, y1, z))` metres: ends at the min and max of the triangle vertices along the unit plan `direction`, on the line through their mean across it; `z` the lowest vertex when `None`.
  - `get_ifc_wall_geometry(wall, ifc_file)` → `(GeometricVector, GeometricVector)` in metres, project coordinates, or `(None, None)`. Order: axis representation → placement origin + X direction × `Length` quantity → `axis_from_faces(get_faces(wall), placement X direction)`. No 1 m fallback.
  - `GenericWall.start_point` / `end_point` set for curtain walls too (from `axis_from_faces` over the parts' faces, direction `(-ny, nx)` perpendicular to the facade normal).

- [ ] **Step 1: Write the failing tests**

Add after `WallElevationTest` in `test_ifc_extractors.py`:

```python
def wall_with_axis(f, context, origin, x_axis, points):
    """A wall placed at `origin` along `x_axis` whose Axis representation is
    the polyline `points` (local (x, y) pairs, file units)."""
    wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, place(f, origin, x_axis), None, None, None)
    poly = f.createIfcPolyline([f.createIfcCartesianPoint((float(x), float(y))) for x, y in points])
    rep = f.createIfcShapeRepresentation(context, "Axis", "Curve2D", [poly])
    wall.Representation = f.createIfcProductDefinitionShape(None, None, [rep])
    return wall


@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class WallAxisTest(TempDirTest):
    """A wall's axis comes out in metres, in project coordinates, with its real length."""

    def ends(self, wall, f):
        start, end = ifc_extractors.get_ifc_wall_geometry(wall, f)
        return (round(start.x, 6), round(start.y, 6), round(start.z, 6)), (round(end.x, 6), round(end.y, 6), round(end.z, 6))

    def test_an_axis_polyline_is_placed_and_scaled(self):
        import ifcopenshell.api.context
        f = new_file("MILLI")
        model = ifcopenshell.api.context.add_context(f, context_type="Model")
        wall = wall_with_axis(f, model, (1000, 2000), (0, 1), [(0, 0), (5000, 0)])
        self.assertEqual(self.ends(wall, f), ((1.0, 2.0, 0.0), (1.0, 7.0, 0.0)))

    def test_a_bent_axis_runs_from_its_first_to_its_last_point(self):
        import ifcopenshell.api.context
        f = new_file(None)
        model = ifcopenshell.api.context.add_context(f, context_type="Model")
        wall = wall_with_axis(f, model, (0, 0), (1, 0), [(0, 0), (2, 0), (2, 3)])
        self.assertEqual(self.ends(wall, f), ((0.0, 0.0, 0.0), (2.0, 3.0, 0.0)))

    def test_without_an_axis_the_length_quantity_is_used(self):
        f = new_file("MILLI")
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, place(f, (3000, 0), (1, 0)), None, None, None)
        add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 4000.0), ("Width", 200.0)])
        self.assertEqual(self.ends(wall, f), ((3.0, 0.0, 0.0), (7.0, 0.0, 0.0)))

    def test_without_quantities_the_geometry_is_measured(self):
        f, body = geometry_file()
        wall = wall_with_geometry(f, body, "w", (1, 1), 0.5, length=4.0, height=3.0)
        start, end = ifc_extractors.get_ifc_wall_geometry(wall, f)
        self.assertAlmostEqual(start.x, 1.0, places=3)
        self.assertAlmostEqual(end.x, 5.0, places=3)
        self.assertAlmostEqual(start.y, end.y, places=6)
        self.assertAlmostEqual(start.z, 0.5, places=3)

    def test_a_wall_with_nothing_to_go_on_has_no_axis(self):
        f = new_file(None)
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, None, None, None, None)
        self.assertEqual(ifc_extractors.get_ifc_wall_geometry(wall, f), (None, None))

    def test_a_curtain_wall_gets_an_axis_along_its_facade(self):
        path = os.path.join(self.tmp, "house.ifc")
        curtain_house().write(path)
        model = ifc_extractors.extract_building_model_from_ifc(path)
        cw = next(w for w in model.walls if w.name == "glazing")
        xs = sorted([cw.start_point.x, cw.end_point.x])
        self.assertAlmostEqual(xs[0], 2.0, places=2)
        self.assertAlmostEqual(xs[1], 6.05, places=2)
        self.assertAlmostEqual(cw.start_point.y, cw.end_point.y, places=6)
```

In `SampleFileTest`, add:

```python
    def test_the_sample_house_walls_have_axes_as_long_as_their_lengths(self):
        import math
        path = os.path.join(ROOT, "examples", "IFC", "Building-Architecture.ifc")
        model = ifc_extractors.extract_building_model_from_ifc(path)
        lengths = sorted(round(math.hypot(w.end_point.x - w.start_point.x, w.end_point.y - w.start_point.y), 1)
                         for w in model.walls)
        self.assertEqual(lengths, [1.8, 3.8, 4.2, 6.0])
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests -k WallAxisTest -k have_axes 2>&1 | tail -20`
Expected: FAIL / ERROR (`get_ifc_wall_geometry() takes 1 positional argument`, `ends` off by the file scale or 1 m stubs).

- [ ] **Step 3: Implement the axis sources in `ifc_extractors.py`**

Add `get_axis_points(wall)`, `to_project(matrix, point, length_scale)` and `axis_from_faces(faces, direction, z=None)` next to `get_wall_axis`; rewrite `get_ifc_wall_geometry(wall, ifc_file)` to try the three sources in order (the matrix from `get_local_placement`, the scale from `get_unit_scales`; a placement is optional for the faces source, which then has no direction and returns `(None, None)`); update the call at ~339 to pass `ifc_file`; in `extract_ifc_curtain_wall_to_generic`, after `nx, ny` are settled, set `generic_wall.start_point, generic_wall.end_point` from `axis_from_faces(all parts' triangles, (-ny, nx))`. Remove the 1 m fallback and its comment.

- [ ] **Step 4: Run the whole Python suite**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -5`
Expected: `OK` with 57 tests (50 + 7).

- [ ] **Step 5: Commit**

```bash
git add pyrevit-extension/Makit.extension/lib/ifc_extractors.py pyrevit-extension/tests/test_ifc_extractors.py docs/superpowers/specs/2026-11-23-ifc-isometric-preview-design.md docs/superpowers/plans/2026-11-23-ifc-isometric-preview.md
git commit -m "Extract real wall axes in metres from IFC files" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Every wall and window length in metres; the standalone view uses elevations

**Files:**
- Modify: `pyrevit-extension/Makit.extension/lib/ifc_extractors.py` (`extract_ifc_wall_to_generic` quantities at ~370–385; `extract_ifc_window_to_generic` at ~822–885)
- Modify: `pyrevit-extension/Makit.extension/lib/visualization_utils.py:96–140` (`create_projected_view`)
- Modify: `pyrevit-extension/Makit.extension/lib/geometry_models.py` (`GenericWall`/`GenericWindow` field comments: metres)
- Modify: `examples/IFC/custom_analysis.py:67`
- Test: `pyrevit-extension/tests/test_ifc_extractors.py`, create `pyrevit-extension/tests/test_visualization_utils.py`

**Interfaces:**
- Consumes: Task 1's `get_ifc_wall_geometry(wall, ifc_file)`.
- Produces: `GenericWall.height`, `.length`, `.width` in metres (`length` = the axis length when there is an axis, else the scaled `Length` quantity; `Width` never overwrites it); `GenericWindow.height`, `.width`, `.position` in metres (`OverallHeight`/`OverallWidth` scaled too; the area fallback is `height * width` with no further scale). `create_projected_view` builds a wall face from `bottom`/`top` when both are set, else from the axis z plus `height` (3.0 when `height` is 0).

- [ ] **Step 1: Write the failing tests**

Add to `test_ifc_extractors.py` after `WallAxisTest`:

```python
@unittest.skipIf(ifcopenshell is None, "needs ifcopenshell")
class LengthUnitsTest(TempDirTest):
    """Format 3 writes every wall and window length in metres."""

    def test_wall_dimensions_are_scaled_and_length_is_the_axis(self):
        import ifcopenshell.api.context
        f = new_file("MILLI")
        model = ifcopenshell.api.context.add_context(f, context_type="Model")
        wall = wall_with_axis(f, model, (0, 0), (1, 0), [(0, 0), (6404, 0)])
        add_quantities(f, wall, "Qto_WallBaseQuantities",
                       lengths=[("Height", 2700.0), ("Length", 6404.0), ("Width", 202.0)])
        w = self.extract(f).walls[0]
        self.assertAlmostEqual(w.height, 2.7)
        self.assertAlmostEqual(w.length, 6.404)
        self.assertAlmostEqual(w.width, 0.202)

    def test_a_wall_without_an_axis_takes_the_scaled_length_quantity(self):
        f = new_file("MILLI")
        wall = f.createIfcWall(ifcopenshell.guid.new(), None, "w", None, None, None, None, None, None)
        add_quantities(f, wall, "Qto_WallBaseQuantities", lengths=[("Length", 3200.0), ("Width", 202.0)])
        w = self.extract(f).walls[0]
        self.assertAlmostEqual(w.length, 3.2)
        self.assertAlmostEqual(w.width, 0.202)

    def test_window_size_and_position_are_scaled(self):
        f = new_file("MILLI")
        window = f.createIfcWindow(ifcopenshell.guid.new(), None, "win", None, None, place(f, (1500, 0), (1, 0)),
                                   None, None, 2420.0, 4835.0, None, None, None)
        win = self.extract(f).windows[0]
        self.assertAlmostEqual(win.height, 2.42)
        self.assertAlmostEqual(win.width, 4.835)
        self.assertAlmostEqual(win.position.x, 1.5)
        self.assertAlmostEqual(win.area, 11.7, places=1)
```

Create `pyrevit-extension/tests/test_visualization_utils.py`:

```python
"""The standalone visualisation builds wall faces from the extraction's metres."""
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "pyrevit-extension", "Makit.extension", "lib"))

import geometry_models
import visualization_utils


def wall(start, end, bottom=None, top=None, height=0.0):
    w = geometry_models.GenericWall("w", "wall")
    w.start_point = geometry_models.GeometricVector(*start)
    w.end_point = geometry_models.GeometricVector(*end)
    w.bottom, w.top, w.height = bottom, top, height
    return w


class ProjectedViewTest(unittest.TestCase):

    def test_a_face_stands_from_bottom_to_top(self):
        view = visualization_utils.create_projected_view([wall((0, 0, 5), (4, 0, 5), bottom=0.0, top=3.0)], "South", [])
        points = view["faces"][0]["points"]
        self.assertEqual(points[0], visualization_utils.isometric_project(0, 0, 0))
        self.assertEqual(points[2], visualization_utils.isometric_project(4, 0, 3))

    def test_without_elevations_the_axis_z_and_height_are_used(self):
        view = visualization_utils.create_projected_view([wall((0, 0, 5), (4, 0, 5), height=2.0)], "South", [])
        points = view["faces"][0]["points"]
        self.assertEqual(points[0], visualization_utils.isometric_project(0, 0, 5))
        self.assertEqual(points[3], visualization_utils.isometric_project(0, 0, 7))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests -p "test_*.py" 2>&1 | tail -15`
Expected: the three `LengthUnitsTest` cases fail on unscaled values; `test_a_face_stands_from_bottom_to_top` fails (face built from z 5 and height 3.0).

- [ ] **Step 3: Scale the extractor's lengths**

In `extract_ifc_wall_to_generic`: read `length_scale` once; quantities: `Height` → `height * length_scale`; `Width` → `width * length_scale`; otherwise `Length` → `length * length_scale`; after the axis is set, `generic_wall.length = math.hypot(end.x - start.x, end.y - start.y)` when both ends exist. In `extract_ifc_window_to_generic`: scale `height`, `width`, `OverallHeight`, `OverallWidth` and `position` (each coordinate) by `length_scale`; the area fallback becomes `height * width`. Update the field comments in `geometry_models.py` (`# metres`).

- [ ] **Step 4: Make `create_projected_view` use elevations and fix `custom_analysis.py`**

In `create_projected_view`, replace the `z1/z2/height` block: when `wall.bottom is not None and wall.top is not None`, `z1 = z2 = wall.bottom` and `height = wall.top - wall.bottom`; else the axis z values and `wall.height if wall.height > 0 else 3.0`. In `custom_analysis.py:67` print `wall.length` without `/ 1000`.

- [ ] **Step 5: Run the Python suite**

Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -5`
Expected: `OK`, 62 tests.

- [ ] **Step 6: Commit**

```bash
git add pyrevit-extension/Makit.extension/lib/ifc_extractors.py pyrevit-extension/Makit.extension/lib/visualization_utils.py pyrevit-extension/Makit.extension/lib/geometry_models.py pyrevit-extension/tests/test_ifc_extractors.py pyrevit-extension/tests/test_visualization_utils.py examples/IFC/custom_analysis.py
git commit -m "Write every wall and window length in metres" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Format 3 and the re-extracted samples

**Files:**
- Modify: `pyrevit-extension/Makit.extension/lib/geometry_models.py:4–6` (`FORMAT_VERSION = 3`, comment)
- Modify: `pyrevit-extension/tests/test_ifc_extractors.py` (`test_to_dict_stamps_format_version_2` → 3)
- Modify: `examples/IFC/Ifc4_ARC.json`, `examples/IFC/Ifc2x3_Duplex_Architecture.json`, `examples/IFC/building-model.json` (regenerated)
- Modify: `crates/makit-tools/src/building_model.rs:1181` (`(1, 2)` → `(1, 3)`)

**Interfaces:**
- Produces: extractions with `metadata.formatVersion: 3`, every wall with `startPoint`/`endPoint` in metres (or `null`), lengths in metres.

- [ ] **Step 1: Update the format test and verify it fails**

Rename `test_to_dict_stamps_format_version_2` to `test_to_dict_stamps_format_version_3` and assert `3` twice. Run: `.venv/bin/python -m unittest discover -s pyrevit-extension/tests -k format_version_3 2>&1 | tail -5`
Expected: FAIL (`2 != 3`).

- [ ] **Step 2: Bump `FORMAT_VERSION` to 3**

Comment: "Version 3 added wall axes and put every wall and window length in metres."

- [ ] **Step 3: Record the analysis figures before re-extracting**

Run, for each of the three JSONs: `cargo run -q -p makit -- analyze examples/IFC/<name>.json > <scratchpad>/<name>.before.txt`
(`building-model.json`, `Ifc2x3_Duplex_Architecture.json`, `Ifc4_ARC.json`).

- [ ] **Step 4: Re-extract the samples**

Run: `MAKIT_PYTHON=.venv/bin/python cargo run -q -p makit -- extract examples/IFC/Building-Architecture.ifc --output examples/IFC/building-model.json --force`, then the same for `examples/IFC/Ifc4_ARC.ifc` and `examples/IFC/Ifc2x3_Duplex_Architecture.ifc` with their default `<name>.json` output and `--force` (check the exact flag names with `cargo run -q -p makit -- extract --help`). Expected: each JSON now says `"formatVersion": 3`; `python3 -I -c` counting walls with non-null `startPoint` gives 4 / 56 / 57.

- [ ] **Step 5: Confirm the figures did not move**

Run the three `analyze` commands into `<name>.after.txt` and `diff` each pair. Expected: no differences (areas, WWR, envelope). If `analyze` prints a format note, it is the only acceptable change.

- [ ] **Step 6: Flip the Rust sample assertion**

`building_model.rs:1181`: `assert_eq!((house.storeys.len(), house.format_version), (1, 3));`.

- [ ] **Step 7: Run both suites**

Run: `cargo test -q 2>&1 | grep "test result"` and `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -3`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add pyrevit-extension/Makit.extension/lib/geometry_models.py pyrevit-extension/tests/test_ifc_extractors.py examples/IFC/*.json crates/makit-tools/src/building_model.rs
git commit -m "Stamp format 3 and re-extract the sample models" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Rust model reads the axes; format 2 is stale

**Files:**
- Modify: `crates/makit-tools/src/building_model.rs` (`Vec3` ~73, `GenericWall` ~33–71, `BuildingModel` impl ~186, `to_wall_data` ~609–625)
- Modify: `crates/makit-tools/src/ifc.rs:121–133` (`is_stale`) and its test `staleness_tolerates_a_checkout_and_old_formats` (~370)
- Modify: `crates/makit-tools/src/revit/models.rs:16–26` (`WallData` doc comments: metres)
- Test: the `tests` modules of `building_model.rs` and `ifc.rs`

**Interfaces:**
- Produces:
  - `Vec3` derives `Copy, PartialEq` as well.
  - `GenericWall { pub start_point: Option<Vec3>, pub end_point: Option<Vec3> }` (`#[serde(default, rename = "startPoint")]` / `"endPoint"`), metres, project coordinates, format 3.
  - `impl GenericWall { pub fn axis(&self) -> Option<(Vec3, Vec3)> }` — both ends present and more than 0.001 apart in plan.
  - `impl BuildingModel { pub fn has_axes(&self) -> bool }` — `format_version() >= 3 && walls.iter().any(|w| w.axis().is_some())`.
  - `ifc::is_stale`: `format_version_of(json) < 3` is stale.

- [ ] **Step 1: Write the failing tests**

In `building_model.rs` tests:

```rust
    #[test]
    fn wall_axes_load_and_degenerate_ones_are_none() {
        let model: BuildingModel = serde_json::from_str(
            r#"{ "walls": [
                { "startPoint": {"x": 1.0, "y": 2.0, "z": 0.0}, "endPoint": {"x": 1.0, "y": 7.0, "z": 0.0} },
                { "startPoint": null, "endPoint": null },
                { "startPoint": {"x": 0.0, "y": 0.0, "z": 0.0}, "endPoint": {"x": 0.0005, "y": 0.0, "z": 0.0} }
            ], "metadata": { "formatVersion": 3 } }"#,
        )
        .unwrap();
        let (start, end) = model.walls[0].axis().unwrap();
        assert_eq!((start.y, end.y), (2.0, 7.0));
        assert_eq!(model.walls[1].axis(), None);
        assert_eq!(model.walls[2].axis(), None);
        assert!(model.has_axes());
    }

    #[test]
    fn has_axes_needs_format_three_and_one_axis() {
        let two: BuildingModel = serde_json::from_str(
            r#"{ "walls": [{ "startPoint": {"x": 0.0, "y": 0.0, "z": 0.0}, "endPoint": {"x": 4.0, "y": 0.0, "z": 0.0} }],
                 "metadata": { "formatVersion": 2 } }"#,
        )
        .unwrap();
        assert!(!two.has_axes());
        let bare: BuildingModel =
            serde_json::from_str(r#"{ "walls": [{}], "metadata": { "formatVersion": 3 } }"#).unwrap();
        assert!(!bare.has_axes());
    }

    #[test]
    fn the_sample_extractions_carry_axes() {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/IFC"));
        for name in ["building-model.json", "Ifc2x3_Duplex_Architecture.json", "Ifc4_ARC.json"] {
            let model = load_model(&dir.join(name)).unwrap();
            assert!(model.has_axes(), "{name}");
            assert!(model.walls.iter().all(|w| w.axis().is_some()), "{name}");
        }
    }

    #[test]
    fn wall_data_keeps_the_lengths_in_metres() {
        let model: BuildingModel = serde_json::from_str(
            r#"{ "walls": [{ "length": 6.4, "height": 2.7, "area": 10.0 }], "metadata": { "formatVersion": 3 } }"#,
        )
        .unwrap();
        let data = to_wall_data(&model.walls, 0.0);
        assert_eq!((data[0].length_m, data[0].height_m), (6.4, 2.7));
        assert_eq!(data[0].end_point, [6.4, 0.0, 0.0]);
    }
```

In `ifc.rs` `staleness_tolerates_a_checkout_and_old_formats`: write the fresh model with `Some(3)`, and add after the format-1 check:

```rust
        let two = dir.join("two.json");
        write_model(&two, Some(2)); // predates wall axes
        assert!(is_stale(&ifc, &two));
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p makit-tools axes 2>&1 | tail -20` and `cargo test -q -p makit-tools staleness 2>&1 | tail -5`
Expected: compile errors (`axis`, `has_axes`, `start_point` unknown); staleness assertion fails.

- [ ] **Step 3: Implement**

Add the two fields with doc comments ("Ends of the wall's axis in plan: metres, project coordinates (format 3). `None` when the extractor found no axis."), `axis()`, `has_axes()`; change `to_wall_data` to `length_m: w.length` and `end_point: [w.length, 0.0, 0.0]` and drop the mm comment; `is_stale` compares `< 3` and its doc comment says "predates wall axes (format below 3)"; `WallData` doc comments say metres.

- [ ] **Step 4: Run the tools tests**

Run: `cargo test -q -p makit-tools 2>&1 | grep "test result"`
Expected: `ok`, 100 passed (96 + 4).

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tools/src/building_model.rs crates/makit-tools/src/ifc.rs crates/makit-tools/src/revit/models.rs
git commit -m "Read wall axes from format 3 extractions" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: A z-up isometric projection and rectangle union

**Files:**
- Modify: `crates/makit-geometry/src/drawing.rs:183–196` (`isometric_project`), add `isometric_z_up` beside it; tests at ~285–300
- Modify: `crates/makit-geometry/src/types.rs` (`impl Rectangle`)

**Interfaces:**
- Produces:
  - `pub fn isometric_z_up(x: f64, y: f64, z: f64) -> (f64, f64)` = `((x - y) * COS30, (x + y) * 0.5 + z)`, `COS30 = 0.866_025_403_8`.
  - `isometric_project` becomes y-up on screen: `sy = (x + z) * sin30 + y` (its boxes, in `preview.rs` and `crates/makit-cli/examples/readme_render.rs`, draw right-side-up).
  - `impl Rectangle { pub fn union(&self, other: &Rectangle) -> Rectangle }` — the smallest rectangle covering both.

- [ ] **Step 1: Write the failing tests**

In `drawing.rs` tests, change `test_isometric_project`'s last assertion to `assert!(sy > 0.0)` with the comment "moving up (Y) moves up on a y-up screen", and add:

```rust
    #[test]
    fn z_up_isometric_puts_east_right_north_left_and_heights_straight_up() {
        let (ex, ey) = isometric_z_up(10.0, 0.0, 0.0);
        assert!(ex > 0.0 && ey > 0.0);
        let (nx, ny) = isometric_z_up(0.0, 10.0, 0.0);
        assert!(nx < 0.0 && ny > 0.0);
        assert_eq!(isometric_z_up(0.0, 0.0, 3.0), (0.0, 3.0));
        // A square in plan projects to a 2:1 diamond.
        assert!((ex - 8.660_254_038).abs() < 1e-9 && (ey - 5.0).abs() < 1e-9);
    }
```

In `types.rs` tests:

```rust
    #[test]
    fn union_covers_both_rectangles() {
        let a = Rectangle::new(0.0, 0.0, 2.0, 2.0);
        let b = Rectangle::new(1.0, -1.0, 4.0, 1.0);
        assert_eq!(a.union(&b), Rectangle::new(0.0, -1.0, 5.0, 3.0));
        assert_eq!(a.union(&a), a);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p makit-geometry 2>&1 | tail -20`
Expected: compile errors for `isometric_z_up` and `union`.

- [ ] **Step 3: Implement the three changes**

Doc comment for `isometric_z_up`: "Project a point in project coordinates (x east, y north, z up) into a 30° isometric seen from the south-west: east runs right and up, north left and up, heights straight up. Screen y points up, one unit per metre."

- [ ] **Step 4: Run the geometry tests and the workspace build**

Run: `cargo test -q -p makit-geometry 2>&1 | grep "test result"` then `cargo build -q --examples`
Expected: `ok`, 42 passed; the build is clean.

- [ ] **Step 5: Commit**

```bash
git add crates/makit-geometry/src/drawing.rs crates/makit-geometry/src/types.rs
git commit -m "Add a z-up isometric projection and rectangle union" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: The TUI keeps the loaded model and routes IFC nodes to the isometric

**Files:**
- Modify: `crates/makit-tui/src/msg.rs:20–40` (`IfcOutcome::Analysis`)
- Modify: `crates/makit-tui/src/runtime.rs:276–315` (`load_ifc`) and its tests (~375, ~399)
- Modify: `crates/makit-tui/src/update.rs:510–560` (`ifc_loaded`) and its tests constructing `IfcOutcome::Analysis` (~918, ~1004)
- Modify: `crates/makit-tui/src/app.rs` (`IfcState` ~160–185, `preview_kind` ~305, `preview_data` ~320) and `crates/makit-tui/src/preview.rs:33–41` (`PreviewData`)
- Test: `app.rs` and `runtime.rs` tests

**Interfaces:**
- Consumes: `BuildingModel::has_axes()` (Task 4).
- Produces:
  - `IfcOutcome::Analysis { analysis, envelope, model: BuildingModel, json, extracted, note }`.
  - `IfcState.model: Option<BuildingModel>` (set from the outcome, cleared on every load).
  - `PreviewData<'a>.model: Option<&'a BuildingModel>` and `impl<'a> PreviewData<'a> { pub fn building(&self) -> Option<&'a BuildingModel> }` — the model only when `has_axes()`.
  - `AppState::preview_kind`: for a node whose routed kind is `Logo`: `Building` when `preview_data().building()` is `Some`, else `Walls` when `wwr` is `Some`, else `Logo`.

- [ ] **Step 1: Write the failing tests**

In `app.rs` tests, add a helper and two tests next to `a_loaded_analysis_replaces_the_logo_on_ifc_nodes`:

```rust
    fn model(format_version: u32, with_axis: bool) -> makit_tools::building_model::BuildingModel {
        let wall = if with_axis {
            r#"{ "startPoint": {"x": 0.0, "y": 0.0, "z": 0.0}, "endPoint": {"x": 8.0, "y": 0.0, "z": 0.0}, "bottom": 0.0, "top": 3.0 }"#
        } else {
            "{}"
        };
        serde_json::from_str(&format!(
            r#"{{ "walls": [{wall}], "metadata": {{ "formatVersion": {format_version} }} }}"#
        ))
        .unwrap()
    }

    #[test]
    fn a_loaded_model_with_axes_is_drawn_in_isometric_on_ifc_nodes() {
        let mut s = state();
        s.tree.select(1); // source:ifc
        s.ifc.analysis = Some(analysis());
        s.ifc.model = Some(model(3, true));
        assert_eq!(s.preview_kind(), PreviewKind::Building);
        assert!(s.preview_data().building().is_some());
        assert!(!s.needs_tick());
        s.tree.select(2); // source:revit — the picked model says nothing about it
        assert_eq!(s.preview_kind(), PreviewKind::Building);
        assert!(s.preview_data().model.is_none());
    }

    #[test]
    fn a_model_without_axes_keeps_the_rose() {
        let mut s = state();
        s.tree.select(1);
        s.ifc.analysis = Some(analysis());
        s.ifc.model = Some(model(2, true)); // a format 2 extraction: stubs, never drawn
        assert_eq!(s.preview_kind(), PreviewKind::Walls);
        assert!(s.preview_data().building().is_none());
        s.ifc.model = Some(model(3, false)); // format 3 but nothing to draw
        assert_eq!(s.preview_kind(), PreviewKind::Walls);
    }
```

In `runtime.rs` test `example_ifc_resolves_to_its_json_extraction`, destructure `model` too and add `assert_eq!(model.walls.len(), 4);`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p makit-tui 2>&1 | tail -20`
Expected: compile errors (`model` field unknown on `IfcState`, `IfcOutcome::Analysis`, `PreviewData`).

- [ ] **Step 3: Implement**

Add the `model` field to `IfcOutcome::Analysis` and fill it in `load_ifc` (the already-loaded `model`, moved after `analyze_wwr`/`envelope` borrow it); add `IfcState.model`, clear it in `ifc_loaded` with the rest and store it from the outcome; add `PreviewData.model` and `building()`; fill `model` in `preview_data()` under the same `ifc_section_visible()` filter as `wwr`; rewrite `preview_kind()` per the Interfaces block. Update the two `IfcOutcome::Analysis` constructions in `update.rs` tests with `model: <a model loaded from building-model.json>` (a `sample_model()` helper beside `sample_analysis()`), and `runtime.rs:399`'s pattern with `..`.

- [ ] **Step 4: Run the TUI tests**

Run: `cargo test -q -p makit-tui 2>&1 | grep "test result"`
Expected: `ok`, 193 passed (191 + 2).

- [ ] **Step 5: Commit**

```bash
git add crates/makit-tui/src/msg.rs crates/makit-tui/src/runtime.rs crates/makit-tui/src/update.rs crates/makit-tui/src/app.rs crates/makit-tui/src/preview.rs
git commit -m "Keep the loaded IFC model and route its nodes to the isometric preview" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Draw the model as standing panels in two tones

**Files:**
- Modify: `crates/makit-tui/src/preview.rs` (`record`, `draw_preview`, `title`, `caption`, `content_bounds`, new `Scene`, `BuildingSummary`, `building_summary`, `draw_building`; tests)
- Modify: `crates/makit-tui/src/view.rs:521–575` (`render_preview`) and tests (~1384–1414)
- Modify: `crates/makit-tui/src/node.rs` (`PreviewKind::Building` doc comment)

**Interfaces:**
- Consumes: `isometric_z_up`, `Rectangle::union` (Task 5); `GenericWall::axis()` (Task 4); `PreviewData::building()` (Task 6).
- Produces:
  - `pub struct Scene { pub main: Recording, pub faint: Recording }` (`Debug, Clone, Default, PartialEq`) with `pub fn is_empty(&self) -> bool` and `pub fn bounds(&self) -> Option<Rectangle>` (union of both layers' bounds).
  - `pub fn record(kind: PreviewKind, angle: f64, data: PreviewData) -> Scene`; every kind but a drawable `Building` records into `main` through the existing `draw_preview`; `Building` with `data.building()` calls `draw_building(&mut scene.main, &mut scene.faint, model)`.
  - `pub fn content_bounds(kind: PreviewKind, scene: &Scene) -> Option<Rectangle>` (logo fixed, else `scene.bounds()`).
  - `pub struct BuildingSummary { pub drawn: usize, pub skipped: usize, pub width: f64, pub depth: f64, pub height: f64 }` (`Debug, Clone, Copy, PartialEq, Default`) and `pub fn building_summary(model: &BuildingModel) -> BuildingSummary`: `width`/`depth` are the x/y extents of the drawn walls' axes, `height` the highest `top` minus the lowest `bottom`.
  - `fn draw_building<P: Plotter + ?Sized>(main: &mut P, faint: &mut P, model: &BuildingModel)`: a wall is drawn when `axis()` is `Some` and both `bottom` and `top` are `Some`; its four corners `(start, bottom)`, `(end, bottom)`, `(end, top)`, `(start, top)` go through `isometric_z_up`; the four edges join consecutive corners in that order (the bottom edge first, then back to the start) and go into `faint` when `is_external == Some(false)`, else into `main`. One private helper decides drawability for both `draw_building` and `building_summary`.
  - `title`: `Building` with `data.building()` → `"isometric"`. `caption`: `Building` with a model → the two lines from Global Constraints.
  - `view::render_preview` draws `scene.faint.segments` in `THEME.dim`, calls `ctx.layer()`, then the existing main/toned pass; `expand_points` runs on `scene.main.points`.

- [ ] **Step 1: Write the failing tests**

In `preview.rs` tests, add a model builder and these tests (and switch the existing tests' `rec` uses to `scene.main` where they inspect points, segments or labels, with `content_bounds(kind, &scene)`):

```rust
    /// Walls as (start xy, end xy, bottom, top, exterior); `None` for a wall with no axis.
    fn model(walls: &[Option<((f64, f64), (f64, f64), Option<f64>, Option<f64>, bool)>]) -> BuildingModel {
        let walls: Vec<serde_json::Value> = walls
            .iter()
            .map(|w| match w {
                Some(((x0, y0), (x1, y1), bottom, top, exterior)) => serde_json::json!({
                    "startPoint": {"x": x0, "y": y0, "z": 0.0}, "endPoint": {"x": x1, "y": y1, "z": 0.0},
                    "bottom": bottom, "top": top, "isExternal": exterior
                }),
                None => serde_json::json!({}),
            })
            .collect();
        serde_json::from_value(serde_json::json!({ "walls": walls, "metadata": { "formatVersion": 3 } })).unwrap()
    }

    fn with_model(model: &BuildingModel) -> PreviewData<'_> {
        PreviewData { model: Some(model), ..PreviewData::default() }
    }

    #[test]
    fn a_model_with_axes_is_drawn_as_standing_panels() {
        let m = model(&[
            Some(((0.0, 0.0), (10.0, 0.0), Some(0.0), Some(3.0), true)),
            Some(((5.0, 0.0), (5.0, 8.0), Some(0.0), Some(3.0), false)),
        ]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert_eq!((scene.main.segments.len(), scene.faint.segments.len()), (4, 4));
        // The exterior wall's bottom edge runs east: right and up on screen.
        let (a, b) = scene.main.segments[0];
        assert_eq!(a, (0.0, 0.0));
        assert!((b.0 - 8.660_254_038).abs() < 1e-9 && (b.1 - 5.0).abs() < 1e-9);
        assert!(scene.main.points.is_empty());
        let s = building_summary(&m);
        assert_eq!((s.drawn, s.skipped), (2, 0));
        assert_eq!((s.width, s.depth, s.height), (10.0, 8.0, 3.0));
        assert_eq!(title(PreviewKind::Building, with_model(&m)), "isometric");
        assert_eq!(
            caption(PreviewKind::Building, with_model(&m)),
            vec!["Walls 2 drawn", "10.0 × 8.0 m footprint · 3.0 m high"]
        );
    }

    #[test]
    fn walls_without_an_axis_or_height_are_counted_not_drawn() {
        let m = model(&[
            Some(((0.0, 0.0), (4.0, 0.0), Some(0.0), Some(2.5), true)),
            Some(((0.0, 0.0), (0.0, 4.0), None, Some(2.5), true)),
            None,
        ]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert_eq!(scene.main.segments.len(), 4);
        assert_eq!(building_summary(&m).skipped, 2);
        assert_eq!(
            caption(PreviewKind::Building, with_model(&m))[0],
            "Walls 1 drawn · 2 without axis or height"
        );
    }

    #[test]
    fn an_all_interior_model_still_fills_the_pane() {
        let m = model(&[Some(((0.0, 0.0), (6.0, 0.0), Some(0.0), Some(3.0), false))]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert!(scene.main.is_empty() && !scene.faint.is_empty());
        let bounds = content_bounds(PreviewKind::Building, &scene).unwrap();
        assert!(bounds.width > 0.0 && bounds.height > 0.0);
    }

    #[test]
    fn a_model_without_axes_is_the_illustration() {
        let two: BuildingModel = serde_json::from_str(
            r#"{ "walls": [{ "startPoint": {"x": 0.0, "y": 0.0, "z": 0.0}, "endPoint": {"x": 4.0, "y": 0.0, "z": 0.0} }],
                 "metadata": { "formatVersion": 2 } }"#,
        )
        .unwrap();
        let real = record(PreviewKind::Building, 0.0, with_model(&two));
        assert_eq!(real, record(PreviewKind::Building, 0.0, PreviewData::default()));
        assert_eq!(title(PreviewKind::Building, with_model(&two)), "isometric · illustration");
        assert!(caption(PreviewKind::Building, with_model(&two)).is_empty());
    }

    #[test]
    fn the_sample_house_draws_every_wall() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/IFC/Ifc4_ARC.json");
        let m = makit_tools::building_model::load_model(std::path::Path::new(path)).unwrap();
        let s = building_summary(&m);
        assert_eq!(s.drawn + s.skipped, m.walls.len());
        assert!(s.drawn >= 47, "{s:?}");
        assert!(s.height > 9.0 && s.height < 12.0, "{s:?}");
    }
```

Add `titles_say_which_previews_are_real` a line: `assert_eq!(title(PreviewKind::Building, none), "isometric · illustration");` already holds; keep it. In `view.rs` tests, next to `a_loaded_analysis_is_drawn_as_the_wall_rose`:

```rust
    #[test]
    fn a_loaded_model_is_drawn_in_isometric() {
        let mut s = state(120, 40);
        s.tree.select(1); // source:ifc
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/IFC/Ifc4_ARC.json");
        let model = building_model::load_model(Path::new(path)).unwrap();
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        s.ifc.analysis = Some(building_model::analyze_wwr(&model));
        s.ifc.model = Some(model);
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("Preview · isometric") && !l.contains("illustration")));
        assert!(lines.iter().any(|l| l.contains("Walls ") && l.contains(" drawn")));
        assert!(lines.iter().any(|l| l.contains("m footprint")));
        assert!(has_braille(&lines));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -q -p makit-tui 2>&1 | tail -20`
Expected: compile errors (`Scene`, `building_summary`, `PreviewData.model` in `with_model` is fine after Task 6).

- [ ] **Step 3: Implement `preview.rs`**

`Scene`, `record` returning it, `content_bounds(kind, &Scene)`, the private drawability helper (`fn panel(wall: &GenericWall) -> Option<(Vec3, Vec3, f64, f64)>`), `draw_building`, `building_summary`, the `title` and `caption` arms. Imports: `makit_geometry::drawing::isometric_z_up`, `makit_tools::building_model::{BuildingModel, GenericWall, Vec3}`.

- [ ] **Step 4: Implement `view::render_preview` and the doc comment**

Faint segments in `THEME.dim`, `ctx.layer()`, then the existing pass; `node.rs`: "Isometric wireframe: the picked IFC model's walls on IFC nodes, else an illustration."

- [ ] **Step 5: Run the TUI tests, fmt and clippy**

Run: `cargo test -q -p makit-tui 2>&1 | grep "test result"` then `cargo fmt && cargo clippy -q --all-targets 2>&1 | tail -5`
Expected: `ok`, 199 passed (193 + 6); no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/makit-tui/src/preview.rs crates/makit-tui/src/view.rs crates/makit-tui/src/node.rs
git commit -m "Draw the loaded IFC model as an isometric of its walls" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Docs, counts and a look at the running TUI

**Files:**
- Modify: `docs/IFC_SUPPORT.md` (lines 9–13 staleness; a "Generic Format: Version 3" section after the version 2 one)
- Modify: `docs/WALL_ORIENTATION_ANALYSIS.md:132–172` (Generic Data Format: `startPoint`/`endPoint`, units of `height`/`length`/`width`)
- Modify: `examples/IFC/README.md:41–45` (staleness sentence)
- Modify: `AGENTS.md` (lines 65 and 68 test counts; the `makit-tui` module line's `preview.rs`; the IFC and TUI canvas rows of "Before Modifying Code")
- Modify: `README.md:68` (Previews paragraph)

- [ ] **Step 1: Run every suite and note the counts**

Run: `cargo test -q 2>&1 | grep "test result"`, `.venv/bin/python -m unittest discover -s pyrevit-extension/tests 2>&1 | tail -3`, `python3 -m unittest discover -s scripts/tests 2>&1 | tail -3`
Expected: all green; per-crate counts for AGENTS.md (expected 3 cli, 10 core, 42 geometry, 100 tools, 199 tui = 354; Python 62; runner 3).

- [ ] **Step 2: Write the docs**

- `IFC_SUPPORT.md`: staleness "or the JSON predates wall axes (`metadata.formatVersion` below 3)"; new section: format 3 adds, per wall, `startPoint`/`endPoint` in metres and project coordinates (from the `Axis` curve, else placement and `Length`, else the geometry; curtain walls from their panels; `null` when none), and puts `height`, `length`, `width` and window `height`, `width`, `position` in metres; a JSON snippet with one wall; "the TUI draws a format 3 model in isometric; format 2 files still load for WWR and the envelope".
- `WALL_ORIENTATION_ANALYSIS.md`: add `"startPoint": {"x": 1.0, "y": 2.0, "z": 0.0}, "endPoint": {…}` to the wall example and a bullet: axes and lengths are metres (format 3).
- `examples/IFC/README.md`: "…or predates wall axes (format below 3)".
- `AGENTS.md`: counts; IFC row: "`metadata.formatVersion` 3 marks extractions with wall axes in metres (`startPoint`/`endPoint`) and every length in metres; staleness: …, or format below 3"; TUI canvas row: "the `Building` preview draws the picked model's walls in isometric (`preview::draw_building`, exterior accent over interior dim) when its extraction has axes, else the illustration; IFC nodes fall back to the rose".
- `README.md` Previews: "Load an IFC file and its walls are drawn in isometric on the `ifc` node (`isometric`), exterior walls bright and interior walls dim."

- [ ] **Step 3: Look at it running**

Run `cargo run -q -p makit -- tui` in the Terminal panel (`mcp__terminal__run_in_terminal`, cwd the repository root, `MAKIT_PYTHON=.venv/bin/python`), press `Tab`, move to `Ifc4_ARC.ifc`, `Enter`, wait, then read the screen (`mcp__terminal__read_terminal`). Expected: the Preview header says `isometric`, braille walls, the caption's two lines; `q` quits. Note anything odd in the final report rather than fixing silently.

- [ ] **Step 4: Final checks and commit**

Run: `cargo fmt --check && cargo clippy -q --all-targets && git status --short`
Expected: clean; only this work's files are staged.

```bash
git add docs/IFC_SUPPORT.md docs/WALL_ORIENTATION_ANALYSIS.md examples/IFC/README.md AGENTS.md README.md
git commit -m "Document format 3 and the isometric preview" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```
