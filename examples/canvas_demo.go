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
		{Start: geometry.Point{X: 0, Y: 0}, End: geometry.Point{X: 30, Y: 0}},
		{Start: geometry.Point{X: 30, Y: 0}, End: geometry.Point{X: 30, Y: 25}},
		{Start: geometry.Point{X: 30, Y: 25}, End: geometry.Point{X: 0, Y: 25}},
		{Start: geometry.Point{X: 0, Y: 25}, End: geometry.Point{X: 0, Y: 0}},

		// Interior walls
		{Start: geometry.Point{X: 15, Y: 0}, End: geometry.Point{X: 15, Y: 25}},
		{Start: geometry.Point{X: 0, Y: 12}, End: geometry.Point{X: 15, Y: 12}},

		// Door openings (represented as gaps in walls would be more complex)
		{Start: geometry.Point{X: 5, Y: 0}, End: geometry.Point{X: 8, Y: 3}},     // Door swing
		{Start: geometry.Point{X: 18, Y: 12}, End: geometry.Point{X: 21, Y: 15}}, // Another door
	}

	// Draw the geometry
	geometry.DrawLines(c, walls, 40, 20)

	// Render to terminal
	fmt.Println("Floor Plan Preview:")
	fmt.Println(c.Render())
	fmt.Printf("\nRendered %d wall segments\n", len(walls))
}
