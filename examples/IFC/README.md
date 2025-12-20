# IFC Analysis Examples

This folder demonstrates the **hybrid architecture** working with IFC files.

## Files

- `Building-Architecture.ifc` - Sample IFC4 file (from SketchUp)
- `building-model.json` - Extracted generic format
- `custom_analysis.py` - Example custom analysis script

## Quick Start

### 1. Analyze IFC File Directly

```bash
cd ../../pyrevit-extension/Makit.extension/lib
python3 analyze_ifc.py ../../../examples/IFC/Building-Architecture.ifc
```

**Output:**
```
Total Walls: 4
Total Wall Area: 43.3 sq m
Total Windows: 0
Overall WWR: 0.0%

EAST FACING: 1 walls, 21.2 sq m
WEST FACING: 3 walls, 22.1 sq m
```

### 2. Extract to Generic Format

```bash
python3 analyze_ifc.py Building-Architecture.ifc \
  --extract-only \
  --output building-model.json
```

Creates a platform-agnostic JSON file that can be:
- Analyzed without IFC tools
- Version controlled
- Used by other platforms
- Cached and reused

### 3. Run Custom Analysis

```bash
cd /Users/nmax/code/makit/examples/IFC
python3 custom_analysis.py
```

Shows how to write your own analysis using the generic format.

## The Generic Format

The `building-model.json` file is **platform-independent**:

```json
{
  "walls": [
    {
      "id": "1AQAupaRP1txwK1AGiN61V",
      "name": "house - outer wall - house right front",
      "orientation": {"x": 0.0, "y": -1.0, "z": 0.0},
      "area": 6.3,
      "level": "00 groundfloor",
      "type": "solidwall",
      "properties": {
        "description": "A solid outer wall..."
      }
    }
  ],
  "projectNorth": 0.0,
  "units": "m"
}
```

**Key point**: This same format can come from:
- IFC files (this example)
- Revit (via PyRevit extractor)
- Rhino (future)
- Any other platform

## Sample IFC File

The `Building-Architecture.ifc` file contains:
- **4 walls** from a simple house model
- Exported from SketchUp 2024
- IFC4 format
- Includes descriptions and metadata

All walls are on "00 groundfloor" level with orientations:
- 1 East-facing wall (21.2 sq m)
- 3 West-facing walls (22.1 sq m)

## What This Demonstrates

### ✅ Cross-Platform Analysis
The same analysis code works regardless of source:

```python
# From IFC
ifc_model = extract_building_model_from_ifc('model.ifc')
stats = analyze_wall_orientations(ifc_model)

# From Revit (when integrated)
revit_model = extract_building_model_from_revit(options)
stats = analyze_wall_orientations(revit_model)  # Same function!

# From generic JSON
json_model = BuildingModel.from_dict(json.load(open('model.json')))
stats = analyze_wall_orientations(json_model)  # Same function!
```

### ✅ Offline Analysis
Once extracted to generic format, no specialized tools needed:

```bash
# Extract with IFC tools
python3 analyze_ifc.py model.ifc --extract-only --output model.json

# Analyze anywhere, anytime (no ifcopenshell, no Revit, nothing!)
python3 custom_analysis.py
```

### ✅ Extensibility
Easy to write custom analysis:

```python
from geometry_models import BuildingModel
import json

with open('building-model.json') as f:
    model = BuildingModel.from_dict(json.load(f))

# Your custom logic here
for wall in model.walls:
    if wall.area > 50:
        print("Large wall:", wall.name)
```

## Next Steps

1. **Try with your own IFC files**:
   ```bash
   python3 analyze_ifc.py /path/to/your.ifc
   ```

2. **Write custom analysis**:
   - Copy `custom_analysis.py`
   - Modify for your needs
   - Works with any platform's data

3. **Combine sources**:
   - Extract from Revit → `revit.json`
   - Extract from IFC → `ifc.json`
   - Write script to combine and compare

## Requirements

- Python 3.7+
- ifcopenshell: `pip3 install --break-system-packages ifcopenshell`

## See Also

- `/docs/IFC_SUPPORT.md` - Full IFC documentation
- `/docs/HYBRID_ARCHITECTURE.md` - System overview
- `/docs/WALL_ORIENTATION_ANALYSIS.md` - Analysis guide
