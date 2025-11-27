# Wall Orientation & WWR Analysis

A hybrid analysis system for calculating wall orientations and Window-to-Wall Ratios (WWR) by compass direction.

## Architecture

The system uses a **3-layer hybrid approach** for maximum flexibility:

```
┌─────────────────────────────────────────────┐
│  Layer 1: Platform-Specific Extraction     │
│  (Revit, Rhino, IFC, etc.)                 │
└──────────────┬──────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────┐
│  Layer 2: Generic Geometry Format (JSON)   │
│  - Platform-agnostic                        │
│  - Cacheable & reusable                     │
└──────────────┬──────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────┐
│  Layer 3: Analysis Functions                │
│  - Orientation classification               │
│  - WWR calculations                         │
│  - Reporting                                │
└─────────────────────────────────────────────┘
```

## Usage

### Quick Analysis (Hybrid - Revit Only)

Analyze directly from Revit in one command:

```bash
# Basic analysis
makit exec revit analysis wall-orientations

# Filter by workset
makit exec revit analysis wall-orientations --workset QAL_ENVELOPE

# Filter by wall type
makit exec revit analysis wall-orientations --wall-type Exterior

# Use square feet instead of meters
makit exec revit analysis wall-orientations --unit sqf

# Save detailed JSON results
makit exec revit analysis wall-orientations --output results.json
```

**Output:**
```
==================================================
WALL ORIENTATION & WWR ANALYSIS
==================================================

OVERALL SUMMARY
--------------------------------------------------
Total Walls: 127
Total Wall Area: 1247.3 sq m
Total Windows: 42
Total Window Area: 156.8 sq m
Overall WWR: 11.2%

BY DIRECTION
--------------------------------------------------

NORTH FACING:
  Walls: 28
  Wall Area: 312.5 sq m
  Windows: 12
  Window Area: 42.3 sq m
  WWR: 11.9%

SOUTH FACING:
  Walls: 31
  Wall Area: 298.7 sq m
  Windows: 15
  Window Area: 51.2 sq m
  WWR: 14.6%

...
==================================================
```

### Extract Once, Analyze Many (Generic Format)

For cross-platform workflows or repeated analysis:

#### Step 1: Extract building model to generic format

```bash
makit exec revit analysis extract-model --workset QAL_ENVELOPE --output model.json
```

This creates a platform-agnostic JSON file that can be:
- Analyzed by other tools (Rhino, Grasshopper, etc.)
- Cached and reused
- Version controlled
- Analyzed offline without Revit

#### Step 2: Analyze the generic model

The generic model can now be analyzed by:
- Python scripts using `analysis_engine.py`
- Other CAD platforms
- Custom analysis tools
- Web applications

**Example Python usage:**

```python
from geometry_models import BuildingModel
from analysis_engine import analyze_wall_orientations, generate_orientation_report
import json

# Load generic model
with open('model.json', 'r') as f:
    model_data = json.load(f)

# Analyze
building_model = BuildingModel.from_dict(model_data)
stats = analyze_wall_orientations(building_model)
report = generate_orientation_report(stats)

print(report)
```

## Generic Data Format

The generic building model format is JSON-based and platform-agnostic:

```json
{
  "walls": [
    {
      "id": "123456",
      "name": "Basic Wall: Exterior - CMU",
      "orientation": {"x": 1.0, "y": 0.0, "z": 0.0},
      "area": 45.2,
      "height": 3.5,
      "length": 12.9,
      "width": 0.3,
      "type": "Exterior",
      "workset": "QAL_ENVELOPE",
      "isCurtainWall": false,
      "level": "Level 1",
      "windows": [
        {
          "id": "789012",
          "area": 2.1,
          "height": 1.5,
          "width": 1.4
        }
      ]
    }
  ],
  "projectNorth": 0.523,
  "units": "m"
}
```

## API Endpoints

The PyRevit server exposes three endpoints:

### 1. Hybrid Analysis (Revit-specific, fast)
```
POST /api/analysis/wall-orientations
```

Extract and analyze in one step. Best for Revit-only workflows.

### 2. Extract Building Model (Generic format)
```
POST /api/extraction/building-model
```

Extract to platform-agnostic format. Best for caching and cross-platform use.

### 3. Analyze Generic Model (Platform-agnostic)
```
POST /api/analysis/generic
```

Analyze a pre-extracted model. Works with data from any source.

## File Structure

```
makit/
├── pyrevit-extension/Makit.extension/lib/
│   ├── geometry_models.py          # Generic data models
│   ├── analysis_engine.py          # Platform-agnostic analysis
│   ├── revit_extractors.py         # Revit → Generic conversion
│   ├── geometry_extractor.py       # Original extraction (legacy)
│   └── makit_server.py            # HTTP server with endpoints
│
├── internal/
│   ├── pyrevit/
│   │   ├── models.go              # Go types for API
│   │   └── client.go              # HTTP client methods
│   └── tools/revit/
│       └── revit.go               # Task registration
│
└── docs/
    └── WALL_ORIENTATION_ANALYSIS.md  # This file
```

## Benefits of Hybrid Approach

### For Revit Users
- **Fast**: Single command analysis
- **Familiar**: Works like other makit commands
- **Integrated**: Direct access from CLI

### For Multi-Platform Workflows
- **Portable**: Generic format works everywhere
- **Cacheable**: Extract once, analyze many times
- **Extensible**: Easy to add new analysis types
- **Testable**: Can test analysis with mock data

### For Future Extensions
- **Rhino support**: Can implement rhino_extractors.py
- **IFC support**: Can implement ifc_extractors.py
- **Custom tools**: Any tool can output the generic format

## Example Workflows

### Workflow 1: Quick Revit Analysis
```bash
makit exec revit analysis wall-orientations --workset QAL_ENVELOPE
```

### Workflow 2: Extract and Reuse
```bash
# Extract once
makit exec revit analysis extract-model --output project-a.json

# Analyze multiple times with different filters
python custom_analysis.py --input project-a.json --min-wwr 15
python custom_analysis.py --input project-a.json --direction north
```

### Workflow 3: Cross-Platform
```bash
# Extract from Revit
makit exec revit analysis extract-model --output revit-model.json

# Import to Rhino, add analysis layer
rhino-script add-analysis revit-model.json

# Export from Rhino
rhino-script export --output rhino-model.json

# Analyze the combined model
python analyze_combined.py revit-model.json rhino-model.json
```

## Next Steps

1. Add support for other platforms (Rhino, IFC)
2. Add more analysis types (thermal, daylighting)
3. Create web viewer for generic models
4. Add parametric analysis capabilities

## Technical Details

### Orientation Classification

Walls are classified into 8 compass directions based on their normalized orientation vectors:

- North: ±22.5° from true north
- Northeast: 22.5° to 67.5°
- East: 67.5° to 112.5°
- Southeast: 112.5° to 157.5°
- South: 157.5° to 202.5°
- Southwest: 202.5° to 247.5°
- West: 247.5° to 292.5°
- Northwest: 292.5° to 337.5°

### WWR Calculation

```
WWR = (Window Area / (Wall Area + Window Area)) × 100
```

Calculated separately for each direction and overall.

### Curtain Walls

Curtain walls have their orientation inverted before classification (Revit API quirk).
