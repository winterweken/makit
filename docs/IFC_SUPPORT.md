# IFC Support in Makit

The hybrid architecture now supports **IFC files** in addition to Revit! This demonstrates the power of the generic format - the same analysis engine works with both platforms.

## Quick Start

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
- [ ] Energy analysis integration
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
