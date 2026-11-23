# IFC isometric preview — design

Date: 2026-11-23. Status: approved (builder-checked; its one
question — the Python visualisation helper that also reads the wall axes — is
settled in §1 and §4, together with the unit clean-up it led to).

## Intent

The TUI's Detail pane shows a braille preview for the node under the cursor.
Two of them draw the user's own data (the MURB heating bars, the wall-direction
rose); the rest are stock drawings marked `· illustration`. Loading an IFC file
in the picker gives the rose on IFC nodes and nothing anywhere draws the
building's shape.

This change draws the loaded building itself: every wall as a standing panel,
from its start to its end and from its bottom to its top, in a 30° isometric,
on the IFC nodes once a file is loaded.

The why: makit's premise is seeing your model from the terminal. A picker that
loads a model and then shows a stock picture undercuts that, and one glance at
the shape tells you the right file loaded, with the right storeys, before you
run an analysis on it.

## Goals

1. On IFC-context nodes (the `ifc` source and its options), once a model with
   wall axes is loaded, the preview draws the building: each wall's axis from
   start to end, lifted from its `bottom` to its `top`, projected in a 30°
   isometric with project z up; exterior walls in the accent colour, interior
   walls dim; scaled to the pane like every other preview.
2. The extraction carries each wall's axis in metres, in project coordinates,
   with its real length: from the IFC `Axis` curve when the wall has one,
   else from its placement and `Length` quantity, else measured from its
   geometry. Curtain walls get one too. Every other length the walls and
   windows carry is in metres as well. All three local sample files (the
   SketchUp house, the Duplex, the Revit house) draw recognisably.
3. Honest, like the other previews: the title says `isometric` only for real
   data; the caption says how many walls are drawn and how many could not be;
   a model whose extraction predates axes is never drawn from stubs and shows
   the rose as today; the format version marks the change.
4. Nothing else moves: `wall-orientation-wwr` keeps its rose, `murb-simulate`
   its chart, and `revit`/`architect` nodes their illustration (the loaded
   model describes the picked file, so only nodes that work on that file show
   it, the same rule the rose follows).

## Non-goals

- Floors, roofs and slabs (the extraction has their areas, not outlines),
  windows, doors, storey lines, a compass.
- A plan view, rotation, zoom, storey stepping or any new key. The plan can be
  a later toggle.
- Live Revit or Blender geometry. The drawing comes from the picker's model.
  The live Revit extractor keeps writing Revit's internal units and no axes.
- Re-recording the README GIF.
- Parsing IFC in Rust. IfcOpenShell stays the geometry engine; the TUI reads
  the JSON.

## Anti-goals

- A picture that lies: 1 m stubs where a length was missing, a building in
  millimetres beside heights in metres (a 1000:1 pancake), or a stock drawing
  where the user expects their model.
- Slowing the picker. No tessellation or geometry work in Rust; drawing a
  loaded model is a few hundred line segments per frame.
- Breaking existing extractions. A format 2 file still loads for WWR and the
  envelope; only the drawing needs format 3.

## Constraints

- The extraction already writes `startPoint`/`endPoint` per wall, but in the
  file's units (millimetres for Revit exports; `bottom`/`top` are metres), and
  with a 1 m stub when the wall has no `Length` quantity, which is every one of
  the Duplex's 57 walls. The Rust `GenericWall` has no fields for them, so
  nothing in makit reads them today.
- The walls' `height`, `length` and `width`, and the windows' `height`, `width`
  and `position`, are in file units too. The Revit house's walls say
  `length: 202`, their thickness: the extractor lets a `Width` quantity
  overwrite `Length`. Three readers assume millimetres and divide by 1000:
  `examples/IFC/custom_analysis.py`, the Rust WWR conversion
  (`building_model::to_wall_data`), and, in effect, nothing for the metre
  Duplex, where the division is wrong. `visualization_utils.create_projected_view`
  (run by `analyze_ifc.py` on its default path, writing `/tmp/makit_viz.json`,
  which nothing in the repository reads) builds wall faces from the axes plus
  `height`, so it is consistent today only because both are in file units.
- Where the axes are: the Duplex (IFC2x3) walls carry an `Axis` IfcPolyline in
  local 2D coordinates; the Revit house (IFC4) an `Axis` IfcIndexedPolyCurve in
  local 3D (two points, no arcs; its chord matches `Length` to the
  millimetre); the SketchUp house has no `Axis` representation, only a
  `Length` quantity and a tessellated body; curtain walls have neither, only
  their parts' faces. Axis points are local to the wall's placement, which
  `ifcopenshell.util.placement.get_local_placement` resolves to the project;
  the transformed endpoints land on every wall's tessellated body in both
  files.
- The Revit house's model holds its site: 16–24 m landscape and retaining
  walls, flagged external in the file. They are part of the model and draw
  with it, so the house sits inside its site walls.
- Pane: in a 120×40 terminal the preview is about 60 columns by 12–16 rows,
  120×64 braille dots. Drawings are recorded in dot units and `fit_bounds`
  scales them. The canvas draws all segments in one colour; the logo gets its
  colours by drawing toned segment lists in layers (`logo::segments`).
- Elm: `update` is pure; `runtime::load_ifc` does the loading on a worker and
  `IfcOutcome::Analysis` carries results into `IfcState`.
- `ifc::is_stale` treats a format below 2 as stale. Raising the bar to 3 makes
  every format 2 extraction stale: re-extracted on load when a Python with
  IfcOpenShell is found, kept with a note when none is (existing behaviour, no
  new path). The staleness rule is stated in `ifc.rs`, `AGENTS.md`,
  `docs/IFC_SUPPORT.md` and `examples/IFC/README.md`.
- Tests: Python tests build small IFC4 files with `ifcopenshell.api`; Rust
  tests read the committed extractions in `examples/IFC` and some assert they
  are format 2; AGENTS.md states the test counts.

## Approaches

Which picture:

- **A. Plan view** — walls from above, one storey at a time. Cleanest in
  braille, but needs a key to step storeys or smears the Revit house's two
  floors over each other. Not chosen (it can come later as a toggle).
- **B. Isometric wireframe from wall axes and elevations** — the whole
  building in one picture, no new control, uses the heights the extraction
  already has, and replaces the `isometric · illustration` with the real
  thing. Denser in a small pane, so interior walls are dimmed. **Chosen.**
- **C. Isometric from the tessellated geometry** — write each wall's mesh
  outline into the JSON. More faithful (openings, thickness) but the Revit
  house alone is thousands of triangles, the JSON would balloon, and the
  braille pane cannot show the detail. Rejected; a possible later layer.

Where the drawing's data lives:

- **(i) Keep the loaded `BuildingModel` in `IfcState`** and point the preview
  at it. One load, and a later plan view reuses it. **Chosen.**
- (ii) Precompute a segment list on the worker and carry only that. Lighter
  messages, but the next preview would need another field. Rejected.

What format 3 puts in metres:

- (i) The axes only. The smallest change, but it leaves `height`, `length`
  and `width` in file units beside axes in metres: the mixed format the
  anti-goals name, and the standalone visualisation would then draw the
  pancake. Rejected.
- **(ii) Every length.** Wall axes, `height`, `length`, `width`; window
  `height`, `width`, `position`. A format bump is the one cheap moment to
  change what a field means, and the repository's three readers are updated
  with it. **Chosen (my call).**

## The how

### 1. Extractor: wall axes and units (`pyrevit-extension/Makit.extension/lib/`)

`get_ifc_wall_geometry(wall, ifc_file)` returns the wall's axis as two
`GeometricVector`s in metres, project coordinates, trying in order:

1. **The `Axis` representation**: the first and last point of an
   `IfcPolyline` or `IfcIndexedPolyCurve` (2D points get z = 0), transformed by
   the wall's absolute placement matrix and scaled by the file's length scale
   (`get_unit_scales`). Other curve types fall through.
2. **Placement and `Length`**: the placement origin and its X direction times
   the wall's `Length` quantity, scaled.
3. **The geometry**: project the tessellated faces' vertices (`get_faces`,
   already metres and project coordinates) onto the plan direction along the
   wall (the placement X axis; for a curtain wall, the horizontal
   perpendicular to its facade normal); the min and max along it give the
   ends, the mean across it the line, the wall's `bottom` the z.
4. None of these: start and end stay `None` and are written `null`.

The 1 m fallback is removed. `extract_ifc_curtain_wall_to_generic` sets the
axis by step 3 from the parts' faces it already tessellates. The live Revit
extractor writes no axes (unchanged; its walls read as "without an axis").

Units: format 3 writes every length in metres. For walls, `height` (the
`Height` quantity, scaled), `length` (the axis length when there is one, else
the `Length` quantity, scaled; a `Width` quantity no longer overwrites it) and
`width` (the `Width` quantity, scaled). For windows, `height`, `width` and
`position` (scaled). Areas keep following `--unit`.

`visualization_utils.create_projected_view` builds each wall's face from the
axis and `bottom`/`top`, falling back to the axis z plus `height` only for a
wall without elevations, so the standalone `analyze_ifc.py` picture stays
consistent; its window sizes are metres now as well.

`FORMAT_VERSION` becomes 3. Format 3 means: wall axes, and every wall and
window length, in metres. Nothing else in the dict changes.

### 2. Rust model (`crates/makit-tools/src/building_model.rs`, `ifc.rs`)

- `GenericWall` gains `start_point: Option<Vec3>` and `end_point: Option<Vec3>`
  (`startPoint`, `endPoint`), documented as metres, project coordinates,
  format 3. A helper `GenericWall::axis()` returns `Some((start, end))` only
  when both exist and they are more than 1 mm apart.
- `BuildingModel::has_axes()` (or equivalent): format 3 or later and at least
  one wall with an axis. The TUI draws only then.
- `to_wall_data` stops dividing `length` by 1000 (`length_m: w.length`, the
  `end_point` likewise); the "generic format stores mm" comment goes. The
  fields' doc comments say metres.
- `ifc::is_stale`: a format below 3 is stale. Doc comments and the AGENTS.md
  trap row say "or the JSON predates wall axes (format below 3)".
- The envelope's "predates storeys and slabs" note (`format_version < 2`)
  stays as it is.

### 3. TUI (`crates/makit-tui/src/`)

- `IfcOutcome::Analysis` carries the loaded `model: BuildingModel`;
  `IfcState.model: Option<BuildingModel>` holds it and is cleared with the
  rest on a new load.
- `PreviewData.model: Option<&BuildingModel>`, filled for IFC-context nodes
  under the same rule as `wwr`.
- Routing: `AppState::preview_kind` on a node that would show the logo (the
  `ifc` source and its options): `Building` when the loaded model has axes,
  else `Walls` when there is an analysis (today's rule), else the logo.
  `PreviewKind::for_node` itself does not change.
- `preview.rs`, `Building` with a model: for each wall with an axis and both
  `bottom` and `top`, the four corners (start, bottom), (end, bottom),
  (end, top), (start, top) go through a y-up isometric of project coordinates
  (x east, y north, z up), viewer at the south-west looking north-east:
  `sx = (x − y)·cos 30°`, `sy = (x + y)·sin 30° + z`, so south and west
  facades face the viewer and heights stand upright. Four edges per wall.
  Exterior walls (`is_external != Some(false)`) draw in the accent colour,
  interior walls in the dim colour, interior first so exterior edges win where
  they cross — the same layering the logo uses for its kit and hero block.
  Walls with an axis but no `bottom`/`top`, or no axis, are not drawn and are
  counted.
- Two tones need the recording to say which segments are faint. The logo
  solves this outside `Recording` with `logo::segments`; this preview needs
  the same split for data drawings. The builder picks the mechanism (a
  `Scene { main, faint }` pair of recordings returned by `record`, or a tone
  per segment); `content_bounds` must cover both tones and the logo path
  must keep working.
- The projection lives in `makit-geometry` (`drawing.rs`) as a pure function
  with tests, next to `isometric_project`. That older function, used by the
  stock illustration, puts +y downwards on a y-up plotter; the builder may fix
  its sign so the illustration draws right-side-up, or leave it.
- Title: `isometric` with a model, `isometric · illustration` without (as
  today). Caption with a model, two lines:
  `Walls 47 drawn · 9 without an axis` (the second part only when some are)
  and `22.6 × 18.1 m footprint · 9.2 m high` (footprint from the axes'
  bounds, height from the lowest `bottom` to the highest `top`).
- No animation: `needs_tick` stays false for it. No points, only segments, so
  `expand_points` has nothing to do.
- Bounds: the recording's own (`content_bounds` default), so the building
  fills the pane.

### 4. Samples, scripts and CLI

- Re-extract the committed samples with the new extractor:
  `examples/IFC/Ifc4_ARC.json`, `Ifc2x3_Duplex_Architecture.json`,
  `building-model.json`. Areas and the envelope do not change; the WWR table
  and envelope numbers before and after must match (`makit analyze` on each).
- `examples/IFC/custom_analysis.py` prints `wall.length` as metres without
  dividing by 1000.
- `makit extract`, `makit analyze` and the actions are unchanged in interface;
  a re-run writes format 3.

### 5. Docs

- `docs/IFC_SUPPORT.md`: a "Generic Format: Version 3" section (wall axes,
  every length in metres, the three axis sources), the version 2 section
  kept; its opening paragraph's staleness rule says format below 3.
- `docs/WALL_ORIENTATION_ANALYSIS.md`, Generic Data Format: `startPoint` and
  `endPoint` described, and the units of `height`, `length`, `width`.
- `examples/IFC/README.md`: its staleness sentence says format below 3.
- `AGENTS.md`: the IFC trap row (format 3, staleness below 3), the TUI canvas
  row (the `Building` preview draws the picked model on IFC nodes), the
  module list (`preview.rs`), test counts.
- `README.md`, the Previews paragraph: loading an IFC draws its walls in
  isometric.

### 6. Tests

Python (`pyrevit-extension/tests/test_ifc_extractors.py`):

- A wall with an `Axis` polyline, placed off the origin and rotated, in a
  millimetre file: start and end come out in metres at the right project
  coordinates, with the polyline's length.
- A wall with only a `Length` quantity: placement origin plus direction times
  length, in metres.
- A wall with neither (the existing geometry-measured wall): an axis of
  length ≈ 4.0 along its placement.
- A curtain wall (`curtain_house`): an axis along its facade, as long as it.
- A millimetre file's wall `height`, `length` and `width`, and a window's
  `height`, `width` and `position`, come out in metres; `length` is the axis
  length when a `Width` quantity is present too.
- `create_projected_view` on a model with `bottom`/`top`: each face stands
  `top − bottom` high at the axis's ends.
- `to_dict` stamps format 3; the sample house: every `IfcWall` has an axis
  whose length matches its `Length` quantity to a millimetre.

Rust:

- `building_model`: axes parse (present and `null`), `axis()` rejects a
  degenerate pair, `has_axes()` on format 2 and 3; `to_wall_data` keeps
  metres; `is_stale` on a format 2 JSON; the existing assertions that the
  samples are format 2 move to 3.
- `preview`: a two-wall model (one exterior, one interior) records eight
  segments split by tone, bounds cover both; a model without axes falls back
  to the illustration; title and caption lines, including the "without an
  axis" count.
- `app`: the `ifc` node shows `Building` once a model with axes is loaded,
  `Walls` for a format 2 model with an analysis, and unrelated nodes never
  show the model.
- `view`: after loading `Ifc4_ARC.json` the header reads
  `Preview · isometric` and the caption names the wall count.
- `runtime::load_ifc`: the outcome carries the model.
- `makit-geometry`: the projection's axes (east right and up, north left and
  up, z straight up).
- AGENTS.md test counts updated.

## Left to the builder

- The two-tone mechanism (see §3) and whether to fix `isometric_project`.
- Exact caption wording within the two lines above, and what the caption
  says when every wall is drawn.
- Tolerances: a degenerate axis is under 1 mm; a wall's axis z is the axis's
  own z in metres (informational; the drawing uses `bottom`/`top`).
- Branch: `ifc-isometric` off `dev`, which is clean. The untracked local
  files in the tree (`murb_results.*`, `results/`, `examples/EPW/`) are the
  user's and stay uncommitted.
