package geometry

import (
	"testing"
)

func TestGetBounds(t *testing.T) {
	tests := []struct {
		name     string
		lines    []Line
		expected Rectangle
	}{
		{
			name:     "Empty lines",
			lines:    []Line{},
			expected: Rectangle{0, 0, 1, 1},
		},
		{
			name: "Single line",
			lines: []Line{
				{Start: Point{X: 0, Y: 0}, End: Point{X: 10, Y: 10}},
			},
			expected: Rectangle{0, 0, 10, 10},
		},
		{
			name: "Multiple lines",
			lines: []Line{
				{Start: Point{X: 0, Y: 0}, End: Point{X: 10, Y: 0}},
				{Start: Point{X: 10, Y: 0}, End: Point{X: 10, Y: 10}},
				{Start: Point{X: 10, Y: 10}, End: Point{X: 0, Y: 10}},
				{Start: Point{X: 0, Y: 10}, End: Point{X: 0, Y: 0}},
			},
			expected: Rectangle{0, 0, 10, 10},
		},
		{
			name: "Negative coordinates",
			lines: []Line{
				{Start: Point{X: -5, Y: -5}, End: Point{X: 5, Y: 5}},
			},
			expected: Rectangle{-5, -5, 10, 10},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := GetBounds(tt.lines)
			if got != tt.expected {
				t.Errorf("GetBounds() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestScalePoint(t *testing.T) {
	// Setup a standard bounds and canvas
	bounds := Rectangle{0, 0, 100, 100}
	canvasW, canvasH := 200, 200

	tests := []struct {
		name     string
		point    Point
		wantX    int
		wantY    int
	}{
		{
			name:  "Origin",
			point: Point{X: 0, Y: 0},
			// Scale calculation:
			// padding = 2
			// availW = 400 - 4 = 396
			// availH = 800 - 4 = 796
			// scaleX = 396 / 100 = 3.96
			// scaleY = 796 / 100 = 7.96
			// scale = 3.96
			// offsetX = 2 + (396 - 100*3.96)/2 = 2 + 0 = 2
			// offsetY = 2 + (796 - 100*3.96)/2 = 2 + (796 - 396)/2 = 2 + 200 = 202
			// x = (0-0)*3.96 + 2 = 2
			// y = (0-0)*3.96 + 202 = 202
			wantX: 2,
			wantY: 202,
		},
		{
			name:  "Center",
			point: Point{X: 50, Y: 50},
			// x = (50)*3.96 + 2 = 198 + 2 = 200
			// y = (50)*3.96 + 202 = 198 + 202 = 400
			wantX: 200,
			wantY: 400,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotX, gotY := scalePoint(tt.point, bounds, canvasW, canvasH)
			if gotX != tt.wantX || gotY != tt.wantY {
				t.Errorf("scalePoint() = (%v, %v), want (%v, %v)", gotX, gotY, tt.wantX, tt.wantY)
			}
		})
	}
}
