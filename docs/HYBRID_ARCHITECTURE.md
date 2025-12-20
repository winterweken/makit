# Hybrid Architecture Overview

The makit toolkit now has a **3-layer hybrid architecture** for cross-platform building analysis.

## What We Built

```
┌─────────────────────────────────────────────────────────────┐
│                    LAYER 3: CLI Tools                       │
│  • makit CLI (Go)                                           │
│  • analyze_ifc.py (Python standalone)                       │
│  • Custom scripts                                           │
└────────────────────┬────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────┐
│              LAYER 2: Generic Format (JSON)                 │
│                                                             │
│  {                                                          │
│    "walls": [                                               │
│      {                                                      │
│        "id": "123",                                         │
│        "orientation": {"x": 1.0, "y": 0.0, "z": 0.0},      │
│        "area": 45.2,                                        │
│        "windows": [...]                                     │
│      }                                                      │
│    ],                                                       │
│    "projectNorth": 0.523,                                   │
│    "units": "m"                                             │
│  }                                                          │
│                                                             │
└────────────────────┬────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────┐
│          LAYER 1: Platform-Specific Extractors              │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   Revit     │  │    IFC      │  │   Rhino     │        │
│  │  Extractor  │  │  Extractor  │  │  Extractor  │        │
│  │             │  │             │  │  (future)   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Files Created

### Python Libraries (`pyrevit-extension/Makit.extension/lib/`)

1. **`geometry_models.py`** - Generic data models
   - `GeometricVector` - 3D vectors
   - `GenericWall` - Platform-agnostic wall
   - `GenericWindow` - Platform-agnostic window
   - `BuildingModel` - Container for entire building

2. **`analysis_engine.py`** - Platform-independent analysis
   - `analyze_wall_orientations()` - Main analysis function
   - `calculate_wwr()` - WWR calculation
   - `classify_orientation()` - Compass direction classifier
   - `generate_orientation_report()` - Text report generator
   - `filter_walls_by_criteria()` - Filtering utility

3. **`revit_extractors.py`** - Revit → Generic conversion
   - `extract_revit_wall_to_generic()` - Convert Revit wall
   - `extract_revit_window_to_generic()` - Convert Revit window
   - `extract_building_model_from_revit()` - Full extraction
   - `analyze_walls_direct_from_revit()` - Hybrid fast path

4. **`ifc_extractors.py`** - IFC → Generic conversion
   - `extract_ifc_wall_to_generic()` - Convert IFC wall
   - `extract_ifc_window_to_generic()` - Convert IFC window
   - `extract_building_model_from_ifc()` - Full extraction
   - `analyze_ifc_walls()` - Hybrid fast path

5. **`analyze_ifc.py`** - Standalone CLI tool
   - Command-line IFC analyzer
   - No server required
   - Works offline

6. **`makit_server.py`** - Updated HTTP server
   - `/api/analysis/wall-orientations` - Revit hybrid analysis
   - `/api/extraction/building-model` - Extract to generic
   - `/api/analysis/generic` - Analyze pre-extracted model

### Go Code (`internal/`)

1. **`pyrevit/models.go`** - Type definitions
   - `WallOrientationOptions` - Analysis options
   - `WallOrientationResponse` - Analysis results
   - `DirectionStats` - Per-direction statistics
   - `BuildingModelExtractionOptions` - Extraction options

2. **`pyrevit/client.go`** - HTTP client methods
   - `AnalyzeWallOrientations()` - Hybrid analysis
   - `ExtractBuildingModel()` - Extract to generic
   - `AnalyzeGenericModel()` - Analyze pre-extracted

3. **`tools/revit/revit.go`** - CLI tasks
   - `wall-orientations` - Quick analysis
   - `extract-model` - Extract to generic format

### Documentation (`docs/`)

1. **`WALL_ORIENTATION_ANALYSIS.md`** - Main documentation
2. **`IFC_SUPPORT.md`** - IFC-specific guide
3. **`HYBRID_ARCHITECTURE.md`** - This file

## Usage Patterns

### Pattern 1: Quick Revit Analysis (Hybrid)

```bash
# Fastest - direct from Revit
makit exec revit analysis wall-orientations --workset QAL_ENVELOPE
```

**When to use**: Quick analysis, Revit is open, no need to cache

### Pattern 2: Extract & Reuse (Generic)

```bash
# Extract once
makit exec revit analysis extract-model --output model.json

# Analyze many times
python custom_analysis.py model.json
python energy_analysis.py model.json
python cost_estimate.py model.json
```

**When to use**: Need to cache, multiple analyses, offline work

### Pattern 3: IFC Analysis (Standalone)

```bash
# No Revit needed!
python analyze_ifc.py model.ifc --output results.json
```

**When to use**: IFC files, no Revit available, batch processing

### Pattern 4: Cross-Platform Comparison

```bash
# Extract from both
makit exec revit analysis extract-model --output revit.json
python analyze_ifc.py exported.ifc --extract-only --output ifc.json

# Compare
python compare.py revit.json ifc.json
```

**When to use**: Validate exports, compare sources, quality control

## Benefits Achieved

### ✅ Platform Independence
- Same analysis code works with Revit, IFC, and future platforms
- No vendor lock-in
- Easy to add new platforms

### ✅ Performance Flexibility
- **Hybrid path**: Fast direct analysis when possible
- **Generic path**: Cacheable, reusable extraction
- Choose the right approach for your workflow

### ✅ Testability
- Analysis functions are pure - no dependencies
- Can test with mock JSON data
- No need for Revit to run tests

### ✅ Extensibility
- Add new analysis types without touching extraction
- Add new platforms without touching analysis
- Clear separation of concerns

### ✅ Interoperability
- Generic format can be read by any tool
- Can mix sources (Revit + IFC + Rhino)
- Works with version control, CI/CD, etc.

## Performance Comparison

| Approach | Speed | Flexibility | Requires App |
|----------|-------|-------------|--------------|
| Hybrid (Revit direct) | ⚡⚡⚡ Fastest | Medium | Yes - Revit |
| Extract → Analyze | ⚡⚡ Fast | ⚡⚡⚡ Highest | No |
| IFC Standalone | ⚡⚡ Fast | High | No |

## Adding a New Platform

To add support for a new platform (e.g., Rhino, ArchiCAD):

### 1. Create extraction layer

```python
# rhino_extractors.py
from geometry_models import GenericWall, BuildingModel

def extract_building_model_from_rhino(rhino_file, options=None):
    model = BuildingModel()

    # Your Rhino-specific code here
    # Convert Rhino objects → GenericWall, GenericWindow

    return model
```

### 2. Analysis works automatically!

```python
from rhino_extractors import extract_building_model_from_rhino
from analysis_engine import analyze_wall_orientations

model = extract_building_model_from_rhino('model.3dm')
stats = analyze_wall_orientations(model)  # Just works!
```

### 3. (Optional) Add Go CLI tasks

Follow the pattern in `internal/tools/revit/revit.go`:

```go
func registerRhinoTasks() {
    category := tool.AddCategory("rhino", "Rhino integration")

    category.AddTask("analyze-walls", "Analyze Rhino walls", func(ctx *registry.TaskContext) error {
        // Call Python extraction via subprocess or HTTP
        return nil
    })
}
```

## Real-World Workflows

### Workflow: Energy Modeling

```bash
# 1. Extract geometry from Revit
makit exec revit analysis extract-model --workset ENVELOPE --output base.json

# 2. Analyze orientations
python analyze_ifc.py base.json  # Works on generic format!

# 3. Export to energy modeling tool
python export_to_energyplus.py base.json --output model.idf

# 4. Run energy simulation
energyplus model.idf weather.epw
```

### Workflow: Design Iterations

```bash
# Version 1
makit exec revit analysis extract-model --output v1.json
git add v1.json && git commit -m "Design iteration 1"

# Version 2 (after changes)
makit exec revit analysis extract-model --output v2.json
git add v2.json && git commit -m "Design iteration 2"

# Compare
python compare_versions.py v1.json v2.json --metric wwr
```

### Workflow: Quality Control

```bash
# Extract from Revit
makit exec revit analysis extract-model --output revit.json

# Export from Revit to IFC, then extract
python analyze_ifc.py model.ifc --extract-only --output ifc.json

# Validate export quality
python validate_export.py revit.json ifc.json
# Checks: geometry accuracy, metadata preservation, etc.
```

## Next Steps

1. **Test with your IFC file**:
   ```bash
   python analyze_ifc.py /path/to/your.ifc
   ```

2. **Try the generic workflow**:
   ```bash
   # Extract
   python analyze_ifc.py model.ifc --extract-only --output model.json

   # Analyze
   python -c "from geometry_models import *; from analysis_engine import *; import json; m = BuildingModel.from_dict(json.load(open('model.json'))); print(analyze_wall_orientations(m))"
   ```

3. **Build custom analysis**:
   - Create `my_analysis.py`
   - Import `geometry_models` and `analysis_engine`
   - Work on generic format - platform independent!

4. **Add new platforms**:
   - Create `{platform}_extractors.py`
   - Follow the pattern from `revit_extractors.py`
   - Analysis code works immediately!

## Questions?

- **Q: Which approach should I use?**
  - A: Start with hybrid (fast). Use generic when you need caching, offline analysis, or cross-platform work.

- **Q: Can I mix Revit and IFC data?**
  - A: Yes! Extract both to generic format, then write a script to combine them.

- **Q: Do I need ifcopenshell?**
  - A: Only for IFC files. Revit extraction doesn't need it.

- **Q: Can I analyze without Revit or IFC tools?**
  - A: Yes! If you have a pre-extracted generic JSON file, you can analyze it with just Python and the analysis_engine.

- **Q: How do I add custom properties to the generic format?**
  - A: Use the `properties` dict in GenericWall/GenericWindow. It's freeform.
