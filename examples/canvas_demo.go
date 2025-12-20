package main

import (
	"fmt"

	"github.com/winteweken/makit/pkg/canvas"
	"github.com/winteweken/makit/pkg/geometry"
)

func main() {
	// Create a large canvas for the "sheet"
	// Width 80 chars, Height 40 chars
	w, h := 80, 40
	c := canvas.NewCanvas(w, h)

	// --- 1. FLOOR PLAN (Top Left) ---
	drawPlan(c, 0, 0, 40, 20)

	// --- 2. ELEVATION (Top Right) ---
	drawElevation(c, 40, 0, 40, 20)

	// --- 3. SECTION / DATA (Bottom) ---
	drawData(c, 0, 20, 80, 20)

	// Render
	fmt.Println(c.Render())
}

func drawPlan(c *canvas.Canvas, offX, offY, w, h int) {
	// Title
	c.DrawText(offX+2, offY+1, "FLOOR PLAN", canvas.ColorWhite)

	// Define walls (Lines)
	// We'll simulate a 10x10m box roughly centered
	
	// Coordinate offsets for rendering
	pxOffX := offX * 2
	pxOffY := offY * 4
	scale := 4.0 // 1 unit = 4 pixels

	// Define geometry in "local" units
	localWalls := []geometry.Line{
		{Start: geometry.Point{X: 5, Y: 5}, End: geometry.Point{X: 15, Y: 5}},
		{Start: geometry.Point{X: 15, Y: 5}, End: geometry.Point{X: 15, Y: 15}},
		{Start: geometry.Point{X: 15, Y: 15}, End: geometry.Point{X: 5, Y: 15}},
		{Start: geometry.Point{X: 5, Y: 15}, End: geometry.Point{X: 5, Y: 5}},
		{Start: geometry.Point{X: 10, Y: 5}, End: geometry.Point{X: 10, Y: 10}}, // Interior
	}

	for _, wall := range localWalls {
		// Manual transform to canvas space
		// Shift by (10, 20) pixels relative to viewport
		l := geometry.Line{
			Start: geometry.Point{X: wall.Start.X*scale + float64(pxOffX) + 10, Y: wall.Start.Y*scale + float64(pxOffY) + 20},
			End:   geometry.Point{X: wall.End.X*scale + float64(pxOffX) + 10, Y: wall.End.Y*scale + float64(pxOffY) + 20},
		}
		
		// Draw thick wall (Cut Geometry - Thickness 2.0)
		geometry.DrawWall(c, l, 2.0, geometry.Rectangle{}, 1, 1, canvas.ColorWhite)
	}
	
	// Draw Windows (Projection/Linework)
	// Make them thinner (Blue) - Lineweight 0.5 (single pixel)
	winStartX := 8.0*scale + float64(pxOffX) + 10
	winEndX := 12.0*scale + float64(pxOffX) + 10
	winY := 15.0*scale + float64(pxOffY) + 20
	
	// TRYING BOX CHARACTERS for window
	// Convert pixel coords to char coords
	cx1 := int(winStartX) / 2
	cy1 := int(winY) / 4
	cx2 := int(winEndX) / 2
	cy2 := int(winY) / 4
	
	// Draw horizontal line using box char
	c.DrawLineChar(cx1, cy1, cx2, cy2, '─', '│', canvas.ColorCyan)

	// Draw thicker window fame/sill if we wanted depth
	
	// DOOR SYMBOL
	// Right wall, around y=10
	// Pixel coords
	doorX := 15.0*scale + float64(pxOffX) + 10
	doorY := 10.0*scale + float64(pxOffY) + 20
	
	// Convert to char coords
	dcx := int(doorX) / 2
	dcy := int(doorY) / 4
	
	// Draw Door Leaf (Open 90 degrees inside -> Horizontal line to left)
	// Using Box Drawing for the leaf
	c.DrawLineChar(dcx, dcy, dcx-2, dcy, '─', '│', canvas.ColorGreen)
	// Draw specific char for Hinge/Jamb if needed, or just part of the line
	// Indicate swing with a simple character
	c.DrawText(offX + int(15.0*scale/2.0) - 2, offY + int(10.0*scale/4.0) + 1, "◟", canvas.ColorGreen) // curve hint

	// North Arrow
	// Use a symbol as requested
	c.DrawText(offX+w-4, offY+2, "↑ N", canvas.ColorRed)
}

func drawElevation(c *canvas.Canvas, offX, offY, w, h int) {
	c.DrawText(offX+2, offY+1, "SOUTH ELEVATION", canvas.ColorWhite)
	
	pxOffX := offX * 2
	pxOffY := offY * 4
	scale := 4.0
	
	startX := float64(pxOffX) + 20
	startY := float64(pxOffY) + 60 // Ground level
	
	width := 10.0 * scale
	height := 6.0 * scale
	roofH := 3.0 * scale
	
	// Draw solid body (White)
	c.FillRect(int(startX), int(startY-height), int(width), int(height), canvas.ColorWhite)
	
	// Draw roof (Polygon, Red)
	roofPoly := []canvas.Point{
		{X: int(startX), Y: int(startY-height)},
		{X: int(startX + width), Y: int(startY-height)},
		{X: int(startX + width/2), Y: int(startY-height-roofH)},
	}
	c.FillPolygon(roofPoly, canvas.ColorRed)
	
	// Draw Windows (Blue - Glazing)
	c.FillRect(int(startX+1*scale), int(startY-4*scale), int(2*scale), int(2*scale), canvas.ColorBlue)
	c.FillRect(int(startX+7*scale), int(startY-4*scale), int(2*scale), int(2*scale), canvas.ColorBlue)
	
	// Door (Green)
	c.FillRect(int(startX+4.5*scale), int(startY-2.5*scale), int(1*scale), int(2.5*scale), canvas.ColorGreen)
	
	// WWR Label
	c.DrawText(offX+2, offY+15, "WWR Analysis:", canvas.ColorYellow)
	c.DrawText(offX+2, offY+16, "Glazing: 20%", canvas.ColorBlue)
	c.DrawText(offX+2, offY+17, "Solid:   80%", canvas.ColorWhite)
}

func drawData(c *canvas.Canvas, offX, offY, w, h int) {
	c.DrawText(offX+2, offY+1, "PROJECT DATA", canvas.ColorMagenta)
	c.DrawText(offX+2, offY+3, "Project: Tiny House Demo", canvas.ColorWhite)
	c.DrawText(offX+2, offY+4, "North:   True North (0°)", canvas.ColorRed)
	
	// Legend
	c.DrawText(offX+40, offY+1, "LEGEND:", canvas.ColorWhite)
	c.DrawText(offX+40, offY+2, "■ Wall (Solid)", canvas.ColorWhite)
	c.DrawText(offX+40, offY+3, "■ Glazing", canvas.ColorBlue)
	c.DrawText(offX+40, offY+4, "■ Roof", canvas.ColorRed)
}
