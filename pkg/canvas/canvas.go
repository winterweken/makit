package canvas

import (
	"strings"
)

// Canvas represents a 2D drawing surface using braille characters
type Canvas struct {
	width  int
	height int
	pixels [][]bool
}

// Braille patterns for different dot combinations
// Each braille character represents 2x4 dots
const (
	brailleBase = 0x2800
)

var brailleDots = [8]rune{
	0x01, 0x08, 0x02, 0x10, 0x04, 0x20, 0x40, 0x80,
}

// NewCanvas creates a new canvas with the given dimensions (in characters)
// Actual pixel resolution is width*2 x height*4 due to braille encoding
func NewCanvas(width, height int) *Canvas {
	pixels := make([][]bool, height*4)
	for i := range pixels {
		pixels[i] = make([]bool, width*2)
	}
	return &Canvas{
		width:  width,
		height: height,
		pixels: pixels,
	}
}

// Set turns on a pixel at the given coordinates
func (c *Canvas) Set(x, y int) {
	if x >= 0 && x < c.width*2 && y >= 0 && y < c.height*4 {
		c.pixels[y][x] = true
	}
}

// Clear clears the canvas
func (c *Canvas) Clear() {
	for y := range c.pixels {
		for x := range c.pixels[y] {
			c.pixels[y][x] = false
		}
	}
}

// DrawLine draws a line from (x0, y0) to (x1, y1) using Bresenham's algorithm
func (c *Canvas) DrawLine(x0, y0, x1, y1 int) {
	dx := abs(x1 - x0)
	dy := abs(y1 - y0)
	sx := 1
	if x0 > x1 {
		sx = -1
	}
	sy := 1
	if y0 > y1 {
		sy = -1
	}
	err := dx - dy

	for {
		c.Set(x0, y0)

		if x0 == x1 && y0 == y1 {
			break
		}

		e2 := 2 * err
		if e2 > -dy {
			err -= dy
			x0 += sx
		}
		if e2 < dx {
			err += dx
			y0 += sy
		}
	}
}

// DrawRect draws a rectangle outline
func (c *Canvas) DrawRect(x, y, w, h int) {
	c.DrawLine(x, y, x+w, y)
	c.DrawLine(x+w, y, x+w, y+h)
	c.DrawLine(x+w, y+h, x, y+h)
	c.DrawLine(x, y+h, x, y)
}

// FillRect draws a filled rectangle
func (c *Canvas) FillRect(x, y, w, h int) {
	for dy := 0; dy <= h; dy++ {
		for dx := 0; dx <= w; dx++ {
			c.Set(x+dx, y+dy)
		}
	}
}

// Render converts the canvas to a string using braille characters
func (c *Canvas) Render() string {
	var sb strings.Builder

	for charY := 0; charY < c.height; charY++ {
		for charX := 0; charX < c.width; charX++ {
			// Each character represents 2x4 pixels
			pixelX := charX * 2
			pixelY := charY * 4

			var dots rune = brailleBase

			// Map pixels to braille dots
			// Braille pattern:
			// 0 3
			// 1 4
			// 2 5
			// 6 7
			dotMap := [8][2]int{
				{0, 0}, {0, 1}, {0, 2}, {1, 0},
				{1, 1}, {1, 2}, {0, 3}, {1, 3},
			}

			for i, pos := range dotMap {
				px, py := pixelX+pos[0], pixelY+pos[1]
				if py < len(c.pixels) && px < len(c.pixels[py]) && c.pixels[py][px] {
					dots |= brailleDots[i]
				}
			}

			sb.WriteRune(dots)
		}
		sb.WriteRune('\n')
	}

	return sb.String()
}

func abs(x int) int {
	if x < 0 {
		return -x
	}
	return x
}
