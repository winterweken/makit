package geometry

import (
	"math"

	"github.com/deadsy/sdfx/sdf"
	v3 "github.com/deadsy/sdfx/vec/v3"
	"github.com/winteweken/makit/pkg/canvas"
)

// SliceParams configures the cross-section slice
type SliceParams struct {
	RotationX float64 // Rotation around X axis (radians)
	RotationY float64 // Rotation around Y axis (radians)
	Depth     float64 // Slice depth along Z after rotation
}

// RenderSDFSlice renders a cross-section of an SDF3 to a braille canvas
// Applies aspect ratio correction for braille cells (2x4 dots)
func RenderSDFSlice(shape sdf.SDF3, canvasWidth, canvasHeight int, params SliceParams) string {
	c := canvas.NewCanvas(canvasWidth, canvasHeight)

	// Get SDF bounding box for scaling
	bb := shape.BoundingBox()
	size := bb.Size()
	center := bb.Center()

	// Pixel resolution (braille: 2 dots wide, 4 dots tall per char)
	pixelWidth := canvasWidth * 2
	pixelHeight := canvasHeight * 4

	// Aspect ratio correction: braille cells are taller than wide
	// Scale Y sampling by 0.5 to compensate
	aspectY := 0.5

	// Distance threshold for boundary detection
	threshold := math.Max(size.X, size.Y) / float64(pixelWidth) * 1.5

	// Rotation matrices
	cosX, sinX := math.Cos(params.RotationX), math.Sin(params.RotationX)
	cosY, sinY := math.Cos(params.RotationY), math.Sin(params.RotationY)

	// Sample the slice plane
	for py := 0; py < pixelHeight; py++ {
		for px := 0; px < pixelWidth; px++ {
			// Map pixel to world coordinates (centered)
			u := (float64(px)/float64(pixelWidth) - 0.5) * size.X * 1.2
			v := (float64(py)/float64(pixelHeight) - 0.5) * size.Y * 1.2 * aspectY

			// Start with point on slice plane at depth
			x, y, z := u, v, params.Depth

			// Apply Y rotation
			x2 := x*cosY - z*sinY
			z2 := x*sinY + z*cosY

			// Apply X rotation
			y2 := y*cosX - z2*sinX
			z3 := y*sinX + z2*cosX

			// Offset to SDF center
			point := v3.Vec{X: x2 + center.X, Y: y2 + center.Y, Z: z3 + center.Z}

			// Evaluate SDF
			dist := shape.Evaluate(point)

			// Draw if near boundary (|dist| < threshold)
			if math.Abs(dist) < threshold {
				c.Set(px, py, canvas.ColorWhite)
			}
		}
	}

	return c.Render()
}

// CreateDemoBox creates a demo SDFX box for the placeholder
func CreateDemoBox() (sdf.SDF3, error) {
	return sdf.Box3D(v3.Vec{X: 2, Y: 2, Z: 2}, 0.1)
}
