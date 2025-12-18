package geometry

import (
	v2 "github.com/deadsy/sdfx/vec/v2"
)

// Point represents a 2D point (aliased to sdfx v2.Vec)
type Point = v2.Vec

// Line represents a line segment
type Line struct {
	Start, End Point
}

// Rectangle represents a rectangle
type Rectangle struct {
	X, Y, Width, Height float64
}

// Room represents a room with walls
type Room struct {
	Name  string
	Walls []Line
	Area  float64
}

// Floor represents a floor plan
type Floor struct {
	Name  string
	Rooms []Room
	Walls []Line
}

// DrawableGeometry can be drawn on a canvas
type DrawableGeometry interface {
	Draw(canvas Canvas)
}

// Canvas interface for drawing
type Canvas interface {
	DrawLine(x0, y0, x1, y1 int)
	DrawRect(x, y, w, h int)
	FillRect(x, y, w, h int)
	Set(x, y int)
}

// Scale and translate a point to fit in canvas coordinates
func scalePoint(p Point, bounds Rectangle, canvasWidth, canvasHeight int) (int, int) {
	// Add padding
	padding := 2
	availWidth := canvasWidth*2 - padding*2
	availHeight := canvasHeight*4 - padding*2

	// Calculate scale to fit both dimensions
	scaleX := float64(availWidth) / bounds.Width
	scaleY := float64(availHeight) / bounds.Height
	scale := scaleX
	if scaleY < scale {
		scale = scaleY
	}

	// Center the geometry
	offsetX := float64(padding) + (float64(availWidth)-bounds.Width*scale)/2
	offsetY := float64(padding) + (float64(availHeight)-bounds.Height*scale)/2

	x := int((p.X-bounds.X)*scale + offsetX)
	y := int((p.Y-bounds.Y)*scale + offsetY)

	return x, y
}

// GetBounds calculates bounding box for a set of lines
func GetBounds(lines []Line) Rectangle {
	if len(lines) == 0 {
		return Rectangle{0, 0, 1, 1}
	}

	minX, minY := lines[0].Start.X, lines[0].Start.Y
	maxX, maxY := lines[0].Start.X, lines[0].Start.Y

	for _, line := range lines {
		if line.Start.X < minX {
			minX = line.Start.X
		}
		if line.Start.Y < minY {
			minY = line.Start.Y
		}
		if line.End.X < minX {
			minX = line.End.X
		}
		if line.End.Y < minY {
			minY = line.End.Y
		}

		if line.Start.X > maxX {
			maxX = line.Start.X
		}
		if line.Start.Y > maxY {
			maxY = line.Start.Y
		}
		if line.End.X > maxX {
			maxX = line.End.X
		}
		if line.End.Y > maxY {
			maxY = line.End.Y
		}
	}

	return Rectangle{
		X:      minX,
		Y:      minY,
		Width:  maxX - minX,
		Height: maxY - minY,
	}
}

// DrawLines draws a set of lines on a canvas
func DrawLines(canvas Canvas, lines []Line, canvasWidth, canvasHeight int) {
	if len(lines) == 0 {
		return
	}

	bounds := GetBounds(lines)

	for _, line := range lines {
		x0, y0 := scalePoint(line.Start, bounds, canvasWidth, canvasHeight)
		x1, y1 := scalePoint(line.End, bounds, canvasWidth, canvasHeight)
		canvas.DrawLine(x0, y0, x1, y1)
	}
}
