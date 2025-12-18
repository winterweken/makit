---
title: Integrate Open Geometry Library
status: todo
---

# Objective
Replace the ad-hoc internal `pkg/geometry` implementation with a robust, open-source geometry library to handle 3D transformations, vector math, and potentially mesh operations more reliably.

# Context
The current `pkg/geometry` contains basic `Point`, `Line`, and `Rectangle` structs. The TUI manually calculates isometric projections using raw math. To support more complex operations (like future analytical features, better transformations, or IFC/Revit data handling), we should adopt a standard library.

# Candidates
1. **github.com/go-gl/mathgl** (mgl64)
   - Pros: Standard for graphics, vectors, matrices. optimized.
   - Cons: Focused on graphics math (Vectors/Matrices) rather than "higher level" geometry (Polygon intersection, etc), but perfect for the TUI visualization.
2. **github.com/paulmach/orb** or **github.com/twpayne/go-geom**
   - Pros: Good for 2D/GIS.
   - Cons: Less focused on 3D CAD-like operations.
3. **github.com/fogleman/gg** (Context) or similar for drawing?
   - We are drawing to a TUI canvas so we just need the math.

# Implementation Plan

## Phase 1: Selection & Setup
- [ ] Confirm with user which library to use (Recommendation: `mathgl` for vector math + custom high-level structs if needed, or a specific CAD-like library if known).
- [ ] Add dependency to `go.mod`.

## Phase 2: Refactor `pkg/geometry`
- [ ] Replace `geometry.Point` (struct { X, Y float64 }) with the library's Vector type (e.g., `mgl64.Vec2` or `mgl64.Vec3`).
- [ ] Refactor `Line`, `Rectangle` to use these new types.
- [ ] Update `GetBounds`, `ScalePoint` functions to work with the new types.
- [ ] Ensure `DrawLines` adapts to the new types.

## Phase 3: Update Consumers
- [ ] Refactor `internal/tui/model.go`:
    - Update `Face` struct.
    - Update `loadIsometricFaces` to use library's Matrix/Vector math for projection (cleaner code).
    - Update `renderViz` and `convertLinesToFaces`.
- [ ] Refactor `scripts/blender/makit_connector.py` (if necessary to match schemas, though JSON usually bridges this).

## Phase 4: Verification
- [ ] Run `go build`.
- [ ] Verify TUI visualization (Isometric view) looks correct.
- [ ] Test with `blender` source again.
