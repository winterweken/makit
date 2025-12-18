package main

import (
	"fmt"

	"github.com/winteweken/makit/pkg/canvas"
	"github.com/winteweken/makit/pkg/geometry"
)

func main() {
	// Create a canvas
	c := canvas.NewCanvas(40, 20)

	// Draw a simple floor plan
	walls := []geometry.Line{
		// Outer walls
		{Start: geometry.Point{0, 0}, End: geometry.Point{30, 0}},
		{Start: geometry.Point{30, 0}, End: geometry.Point{30, 25}},
		{Start: geometry.Point{30, 25}, End: geometry.Point{0, 25}},
		{Start: geometry.Point{0, 25}, End: geometry.Point{0, 0}},

		// Interior walls
		{Start: geometry.Point{15, 0}, End: geometry.Point{15, 25}},
		{Start: geometry.Point{0, 12}, End: geometry.Point{15, 12}},

		// Door openings (represented as gaps in walls would be more complex)
		{Start: geometry.Point{5, 0}, End: geometry.Point{8, 3}},     // Door swing
		{Start: geometry.Point{18, 12}, End: geometry.Point{21, 15}}, // Another door
	}

	// Draw the geometry
	geometry.DrawLines(c, walls, 40, 20)

	// Render to terminal
	fmt.Println("Floor Plan Preview:")
	fmt.Println(c.Render())
	fmt.Printf("\nRendered %d wall segments\n", len(walls))
}
