# Implementation Plan - Integrate Open Geometry Library

## Problem

The project currently relies on a minimal, custom `pkg/geometry` package for 2D/3D operations. This limits our ability to perform complex transformations (like isometric projection) efficiently and robustly. The user has requested to "work with an open geometry library".

## Proposed Solution

Integrate a standard Go geometry/math library. The primary recommendation is **`github.com/go-gl/mathgl` (v2.x)** for handling Vectors (Vec2, Vec3) and Matrices (Mat4, Mat3). This is the standard in the Go ecosystem for graphics-related math, which fits the TUI visualization needs perfectly.

## Critical Files

- `pkg/geometry/types.go`: Will be refactored to wrap or alias `mathgl` types.
- `internal/tui/model.go`: Will updated to use vector math for projection instead of manual calculations.
- `go.mod`: Dependency addition.

## Step-by-Step Plan

### 1. Setup

- [ ] Add `github.com/go-gl/mathgl` to `go.mod`.

### 2. Refactor Core Geometry

- [ ] Modify `pkg/geometry/types.go`:
  - Deprecate/Replace struct `Point` with `mgl64.Vec2` (or `Vec3`).
  - Update helper functions (`ScalePoint`, `GetBounds`) to use the new vector types.

### 3. Update TUI Model

- [ ] In `internal/tui/model.go`:
  - Update `Face` struct to use `mgl64.Vec3` for points.
  - Rewrite `loadIsometricFaces` to use `mgl64.Rotate3D` and projection matrices for cleaner code.
  - Rewrite `renderViz` to adapt to the new types.

### 4. Verify

- [ ] Build and Test `makit`.
- [ ] Verify TUI rendering with Blender data.
