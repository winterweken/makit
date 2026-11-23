# IFC Support in Makit

The hybrid architecture now supports **IFC files** in addition to Revit! This demonstrates the power of the generic format - the same analysis engine works with both platforms.

## Quick Start

### With makit

makit runs the extractor itself. Give it an IFC file and it writes `<name>.json`
beside the file when there is none, or when the extraction is stale: the IFC is
newer than its JSON by more than two seconds (a git checkout writes both within
milliseconds and never counts), or the JSON predates storeys and slabs
(`metadata.formatVersion` below 2).

```bash
makit extract model.ifc                           # write model.json beside it; --force redoes a fresh one
makit analyze model.ifc                           # WWR by direction and the energy envelope, extracting first if needed
makit analyze model.ifc -t envelope --grade -1.5  # the envelope alone, with grade at -1.5 m (project z)
makit exec murb analysis murb-simulate -o model=model.ifc -o epw=weather.epw
```

The TUI picker does the same when you load an IFC file, and `murb-simulate --model`
and `wall-orientation-wwr --input` accept an IFC path too.

Extraction needs a Python with IfcOpenShell. makit looks for one in this order:
`MAKIT_PYTHON` (which must have it; nothing else is tried behind your back), then
`.venv/bin/python` in the repository, then `python3` and `python` on the PATH.
`uv venv .venv --python 3.13 && uv pip install --python .venv/bin/python ifcopenshell`
sets one up. A fresh JSON never needs Python. A stale one is used with a note
when no Python is found. Only a missing one is an error.

### Standalone Python Analysis

The fastest way to analyze an IFC file:

```bash
cd /path/to/makit/pyrevit-extension/Makit.extension/lib

# Basic analysis
python analyze_ifc.py /path/to/your/model.ifc

# Filter by storey
python analyze_ifc.py model.ifc --storey "Level 1"

# Use square feet
python analyze_ifc.py model.ifc --unit sqf

# Save detailed results
python analyze_ifc.py model.ifc --output results.json

# Extract to generic format only
python analyze_ifc.py model.ifc --extract-only --output model.json
```

### Example Output

```
==================================================
WALL ORIENTATION & WWR ANALYSIS
==================================================

OVERALL SUMMARY
--------------------------------------------------
Total Walls: 89
Total Wall Area: 1024.5 sq m
Total Windows: 34
Total Window Area: 128.3 sq m
Overall WWR: 11.1%

BY DIRECTION
--------------------------------------------------

NORTH FACING:
  Walls: 18
  Wall Area: 245.2 sq m
  Windows: 8
  Window Area: 32.1 sq m
  WWR: 11.6%

...
```

## Architecture Flow

```
IFC File → ifc_extractors.py → Generic Format → analysis_engine.py → Results
   ↓
Revit    → revit_extractors.py → Generic Format → analysis_engine.py → Results
   ↓
Rhino    → rhino_extractors.py → Generic Format → analysis_engine.py → Results
```

**Key insight**: All platforms convert to the same generic format, so analysis code is written once!

## Requirements

For IFC support, you need `ifcopenshell`:

```bash
pip install ifcopenshell
```

## Generic Format: Version 2

Extractions written by this version of the extractor carry `metadata.formatVersion: 2` and, next to
`walls` and `windows`:

```json
{
  "storeys": [{ "id": "…", "name": "Level 1", "elevation": 0.0, "floorArea": 180.5, "slabIds": ["…"] }],
  "slabs": [
    { "id": "…", "name": "Floor:Slab on Grade", "kind": "floor", "area": 64.8,
      "top": 0.0, "bottom": -0.13, "storey": "Level 1", "type": "Floor:127mm Slab on Grade",
      "isExternal": false, "properties": { "ifcType": "IfcSlab" } },
    { "id": "…", "name": "Basic Roof:Live Roof", "kind": "roof", "area": 137.4, "top": 6.46, "bottom": 6.0, "storey": "Roof" }
  ],
  "walls": [{ "…": "…", "bottom": -1.25, "top": 3.1 }]
}
```

Readers treat a missing version as 1; such extractions still load for WWR, and the
envelope says they predate storeys and slabs.

## Generic Format Reusability

Once you extract to generic format, you can:

### 1. Mix sources
```bash
# Extract from Revit
makit exec revit analysis extract-model --output revit.json

# Extract from IFC
python analyze_ifc.py model.ifc --extract-only --output ifc.json

# Combine and analyze (custom script)
python combine_models.py revit.json ifc.json --output combined.json
```

### 2. Version control geometry
```bash
git add building-model.json
git commit -m "Geometry snapshot for energy analysis"
```

### 3. Analyze offline
```bash
# No Revit or IFC tools needed!
python -c "
from geometry_models import BuildingModel
from analysis_engine import analyze_wall_orientations
import json

with open('model.json') as f:
    model = BuildingModel.from_dict(json.load(f))

stats = analyze_wall_orientations(model)
print('Overall WWR:', stats['totals']['wwr'], '%')
"
```

### 4. Custom analysis
```python
from geometry_models import BuildingModel
from analysis_engine import filter_walls_by_criteria, analyze_wall_orientations
import json

# Load model
with open('model.json') as f:
    model = BuildingModel.from_dict(json.load(f))

# Filter only large walls
filtered = filter_walls_by_criteria(model, {'minArea': 50})

# Analyze
stats = analyze_wall_orientations(filtered)
print("Large walls WWR:", stats['totals']['wwr'])
```

## IFC-Specific Features

### Property Extraction

The IFC extractor reads standard IFC properties:

- **Quantities**: Height, Width, Length, Area
- **Type Information**: Wall types, window types
- **Spatial Structure**: Building storeys, spaces
- **Relationships**: Window-to-wall hosting

### Orientation Calculation

IFC wall orientation is extracted from the placement matrix (local coordinate system).

### Curtain Wall Detection

Walls with "CURTAIN" in their ObjectType are automatically flagged and have their orientation inverted (same as Revit logic).

### Storeys, Floor Plates and Roofs

The extractor also records what an energy model needs beyond walls and windows:

- **Storeys** (`storeys`): every `IfcBuildingStorey` with its elevation in metres and
  the `floorArea` of its plates.
- **Floor plates** (`slabs` of kind `floor`): every `IfcSlab` that is not a roof, a
  landing or a part of something else. A plate's `area` is the plan footprint of its
  up-facing faces, and it joins the storey whose elevation is within 0.3 m of its top
  face (finish floors sit a few centimetres above a level, structural slabs at it).
  A storey's `floorArea` is the footprint **union** of its plates, so a finish floor
  laid over a structural floor counts once. A plate at no storey's level (a building
  pad buried under the floor, say) keeps `storey: null`; the analyses report it and
  leave it out.
- **Roofs** (`slabs` of kind `roof`): each `IfcRoof` (its own geometry when it has
  one, else its decomposed parts, never both) and each `ROOF` slab outside any
  `IfcRoof`. A slab anywhere under an `IfcRoof`, however deep the aggregation,
  is measured with it and never on its own. A roof's `area` is its up-facing
  surface (faces whose normal has z > 0.5), so a gable counts its slopes, not
  its footprint.
- **Landings** and other slab parts are kept for the record (`landing`, `other`) and
  counted nowhere.
- **Wall elevations**: each wall carries `bottom` and `top` in metres (from its
  geometry, else its placement and Height quantity), for the split above and
  below grade.
- **Windows** take their area from quantities, else from `OverallHeight × OverallWidth`,
  else from their geometry.

Slab classification uses IFC types, aggregation and geometry only, never names.

### Energy Envelope

`makit analyze model.ifc -t envelope` (and `murb-simulate --model`) derive from the
extraction:

| Quantity | From |
|---|---|
| GFA | the sum of the storeys' `floorArea`, listed per storey with plate counts |
| Walls above / below grade | each exterior wall's net area split where it crosses `--grade` (metres, project z, default 0); a wall without elevations counts above grade |
| Windows | windows in exterior walls plus windows in no wall |
| Roof | the roofs' up-facing surface |

Every figure is in square metres whatever unit the extraction used: an extraction
made with `--unit sqf` (areas in square feet, `units: "ft"`) is converted back.

`murb-simulate` fills every area you did not type from the model and prints each with
its source. A zero from the model is an error for GFA, above-grade walls and roof
(a building has all three; the message names the option to type) and a value for
windows and below-grade walls. The breakdown is printed on purpose: a messy model
(the Revit sample house exports building pads and pile caps as slabs) shows up in
it, and a typed option always wins.

## Adding New Platform Support

Want to add Rhino, ArchiCAD, or another platform? Follow this pattern:

### 1. Create `{platform}_extractors.py`

```python
from geometry_models import GenericWall, BuildingModel

def extract_building_model_from_{platform}(input_path, options=None):
    model = BuildingModel()

    # Platform-specific extraction code here
    # Convert platform elements → GenericWall, GenericWindow

    return model
```

### 2. Analysis is automatic!

```python
from analysis_engine import analyze_wall_orientations

model = extract_building_model_from_rhino('model.3dm')
stats = analyze_wall_orientations(model)  # Just works!
```

### 3. (Optional) Add to makit CLI

Add tasks in `internal/tools/{platform}/` following the Revit pattern.

## Comparison: Revit vs IFC

| Feature | Revit | IFC | Generic Format |
|---------|-------|-----|----------------|
| Extraction Speed | Fast | Fast | N/A (pre-extracted) |
| Requires App Running | Yes | No | No |
| Offline Analysis | No | No | Yes |
| Custom Properties | Full Access | Limited | Depends on extraction |
| Geometry Accuracy | Exact | Exact | Depends on extraction |
| Cross-platform | No | Yes | Yes |

## Future Enhancements

- [ ] Add Rhino/Grasshopper support
- [ ] Add DXF/DWG support for 2D plans
- [ ] Web viewer for generic models
- [ ] Batch analysis of multiple files
- [x] Energy analysis integration (MURB envelope from the extraction)
- [ ] Daylighting analysis
- [ ] Cost estimation based on orientations

## Example: Compare Revit vs IFC

```bash
# Extract from Revit
makit exec revit analysis extract-model --output revit-model.json

# Export Revit to IFC, then extract
python analyze_ifc.py exported.ifc --extract-only --output ifc-model.json

# Compare (custom script)
python compare_models.py revit-model.json ifc-model.json
```

This is useful for validating IFC export quality!
