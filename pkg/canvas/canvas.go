package canvas

import (
	"math"
	"sort"
	"strings"
)

// Canvas represents a 2D drawing surface using braille characters
type Canvas struct {
	width  int
	height int
	pixels [][]bool
	colors [][]Color
	// Text layer overrides braille output at character level
	textLayer  [][]rune
	textColors [][]Color
}

// Braille patterns for different dot combinations
// Each braille character represents 2x4 dots
const (
	brailleBase = 0x2800
)

var brailleDots = [8]rune{
	0x01, 0x08, 0x02, 0x10, 0x04, 0x20, 0x40, 0x80,
}

// Color represents an ANSI color code
type Color string

const (
	ColorNone    Color = ""
	ColorReset   Color = "\033[0m"
	ColorRed     Color = "\033[31m"
	ColorGreen   Color = "\033[32m"
	ColorYellow  Color = "\033[33m"
	ColorBlue    Color = "\033[34m"
	ColorMagenta Color = "\033[35m"
	ColorCyan    Color = "\033[36m"
	ColorWhite   Color = "\033[37m"
)

// NewCanvas creates a new canvas with the given dimensions (in characters)
// Actual pixel resolution is width*2 x height*4 due to braille encoding
func NewCanvas(width, height int) *Canvas {
	pixels := make([][]bool, height*4)
	colors := make([][]Color, height*4)
	for i := range pixels {
		pixels[i] = make([]bool, width*2)
		colors[i] = make([]Color, width*2)
	}
	
	textLayer := make([][]rune, height)
	textColors := make([][]Color, height)
	for i := range textLayer {
		textLayer[i] = make([]rune, width)
		textColors[i] = make([]Color, width)
	}

	return &Canvas{
		width:      width,
		height:     height,
		pixels:     pixels,
		colors:     colors,
		textLayer:  textLayer,
		textColors: textColors,
	}
}

// Set turns on a pixel at the given coordinates with a color
func (c *Canvas) Set(x, y int, color Color) {
	if x >= 0 && x < c.width*2 && y >= 0 && y < c.height*4 {
		c.pixels[y][x] = true
		c.colors[y][x] = color
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
func (c *Canvas) DrawLine(x0, y0, x1, y1 int, color Color) {
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
		c.Set(x0, y0, color)

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
func (c *Canvas) DrawRect(x, y, w, h int, color Color) {
	c.DrawLine(x, y, x+w, y, color)
	c.DrawLine(x+w, y, x+w, y+h, color)
	c.DrawLine(x+w, y+h, x, y+h, color)
	c.DrawLine(x, y+h, x, y, color)
}

// FillRect draws a filled rectangle
func (c *Canvas) FillRect(x, y, w, h int, color Color) {
	for dy := 0; dy <= h; dy++ {
		for dx := 0; dx <= w; dx++ {
			c.Set(x+dx, y+dy, color)
		}
	}
}

// Point represents a 2D integer point for canvas operations
type Point struct {
	X, Y int
}

// FillPolygon draws a filled polygon using scanline algorithm
func (c *Canvas) FillPolygon(points []Point, color Color) {
	if len(points) < 3 {
		return
	}

	minY := points[0].Y
	maxY := points[0].Y
	for _, p := range points {
		if p.Y < minY {
			minY = p.Y
		}
		if p.Y > maxY {
			maxY = p.Y
		}
	}

	// Clip to canvas height
	if minY < 0 {
		minY = 0
	}
	if maxY >= c.height*4 {
		maxY = c.height*4 - 1
	}

	for y := minY; y <= maxY; y++ {
		var nodes []int
		j := len(points) - 1
		for i := 0; i < len(points); i++ {
			if (points[i].Y < y && points[j].Y >= y) || (points[j].Y < y && points[i].Y >= y) {
				nodes = append(nodes, points[i].X+(y-points[i].Y)*(points[j].X-points[i].X)/(points[j].Y-points[i].Y))
			}
			j = i
		}

		sort.Ints(nodes)

		for i := 0; i < len(nodes); i += 2 {
			if i+1 >= len(nodes) {
				break
			}
			xStart := nodes[i]
			xEnd := nodes[i+1]
			
			// Clip x
			if xStart < 0 { xStart = 0 }
			if xEnd >= c.width*2 { xEnd = c.width*2 - 1 }
			
			for x := xStart; x <= xEnd; x++ {
				c.Set(x, y, color)
			}
		}
	}
}

// DrawText draws text starting at the given character coordinates
func (c *Canvas) DrawText(x, y int, text string, color Color) {
	if y < 0 || y >= c.height {
		return
	}
	
	runes := []rune(text)
	for i, r := range runes {
		cx := x + i
		if cx >= 0 && cx < c.width {
			c.textLayer[y][cx] = r
			c.textColors[y][cx] = color
		}
	}
}

// DrawArrow draws an arrow from (x0, y0) to (x1, y1) in pixel coordinates
func (c *Canvas) DrawArrow(x0, y0, x1, y1 int, color Color) {
	c.DrawLine(x0, y0, x1, y1, color)
	
	// Simple arrow head
	// Calculate direction vector
	dx := float64(x1 - x0)
	dy := float64(y1 - y0)
	length := math.Sqrt(dx*dx + dy*dy)
	
	if length > 0 {
		// Normalize
		dx /= length
		dy /= length
		
		// Perpendicular vector
		px := -dy
		py := dx
		
		// Arrow head size (approx 3 pixels)
		headLen := 3.0
		
		// Back 3 pixels, then out 2 pixels
		// Tip is at (x1, y1)
		
		// Wing 1
		wx1 := float64(x1) - dx*headLen + px*headLen*0.6
		wy1 := float64(y1) - dy*headLen + py*headLen*0.6
		
		// Wing 2
		wx2 := float64(x1) - dx*headLen - px*headLen*0.6
		wy2 := float64(y1) - dy*headLen - py*headLen*0.6
		
		c.DrawLine(x1, y1, int(wx1), int(wy1), color)
		c.DrawLine(x1, y1, int(wx2), int(wy2), color)
	}
}

// DrawLineChar draws a line using specific characters on the text layer
// Coordinates are in Characters, not Pixels
func (c *Canvas) DrawLineChar(x0, y0, x1, y1 int, hChar, vChar rune, color Color) {
	dx := abs(x1 - x0)
	dy := -abs(y1 - y0)
	sx := -1
	if x0 < x1 {
		sx = 1
	}
	sy := -1
	if y0 < y1 {
		sy = 1
	}
	err := dx + dy
	
	for {
		// Determine character to use based on slope/dominant direction
		char := hChar
		if abs(y1-y0) > abs(x1-x0) {
			char = vChar
		}

		if x0 >= 0 && x0 < c.width && y0 >= 0 && y0 < c.height {
			c.textLayer[y0][x0] = char
			c.textColors[y0][x0] = color
		}

		if x0 == x1 && y0 == y1 {
			break
		}
		e2 := 2 * err
		if e2 >= dy {
			err += dy
			x0 += sx
		}
		if e2 <= dx {
			err += dx
			y0 += sy
		}
	}
}

// Render converts the canvas to a string using braille characters
func (c *Canvas) Render() string {
	var sb strings.Builder
	lastColor := ColorReset

	for charY := 0; charY < c.height; charY++ {
		for charX := 0; charX < c.width; charX++ {
			// Check text layer first
			if r := c.textLayer[charY][charX]; r != 0 {
				textColor := c.textColors[charY][charX]
				if textColor != ColorNone && textColor != lastColor {
					sb.WriteString(string(textColor))
					lastColor = textColor
				} else if textColor == ColorNone && lastColor != ColorReset {
					sb.WriteString(string(ColorReset))
					lastColor = ColorReset
				}
				sb.WriteRune(r)
				continue
			}

			// Each character represents 2x4 pixels
			pixelX := charX * 2
			pixelY := charY * 4

			var dots rune = brailleBase
			var activeColor Color = ColorNone

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
				if py < c.height*4 && px < c.width*2 && c.pixels[py][px] {
					dots |= brailleDots[i]
					// Use the color of the last pixel found for this char block
					if c.colors[py][px] != "" {
						activeColor = c.colors[py][px]
					}
				}
			}

			if activeColor != ColorNone && activeColor != lastColor {
				sb.WriteString(string(activeColor))
				lastColor = activeColor
			} else if activeColor == ColorNone && lastColor != ColorReset {
				sb.WriteString(string(ColorReset))
				lastColor = ColorReset
			}

			// If all dots are set, use a solid block character for better visual density
			if dots == brailleBase+0xFF {
				dots = '█'
			}

			sb.WriteRune(dots)
		}
		sb.WriteRune('\n')
	}
	sb.WriteString(string(ColorReset))

	return sb.String()
}

func abs(x int) int {
	if x < 0 {
		return -x
	}
	return x
}
