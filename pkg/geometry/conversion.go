package geometry

import (
	"math"

	v3 "github.com/deadsy/sdfx/vec/v3"
)

// Constants for unit conversion
const (
	FeetToMeters = 0.3048
	MetersToFeet = 1.0 / FeetToMeters
)

// ToMeters converts a value from the given unit to meters
func ToMeters(value float64, fromUnit string) float64 {
	switch fromUnit {
	case "feet", "ft":
		return value * FeetToMeters
	case "mm":
		return value / 1000.0
	case "cm":
		return value / 100.0
	case "in", "inch":
		return value * 0.0254
	default:
		return value
	}
}

// ConvertPointToMeters scales a point from a source unit to meters
func ConvertPointToMeters(p Point, fromUnit string) Point {
	scale := 1.0
	switch fromUnit {
	case "feet", "ft":
		scale = FeetToMeters
	case "mm":
		scale = 0.001
	case "in", "inch":
		scale = 0.0254
	}

	if scale == 1.0 {
		return p
	}

	return Point{X: p.X * scale, Y: p.Y * scale}
}

// ConvertPoint3ToMeters scales a 3D point from a source unit to meters
func ConvertPoint3ToMeters(p v3.Vec, fromUnit string) v3.Vec {
	scale := 1.0
	switch fromUnit {
	case "feet", "ft":
		scale = FeetToMeters
	case "mm":
		scale = 0.001
	case "in", "inch":
		scale = 0.0254
	}

	if scale == 1.0 {
		return p
	}

	return v3.Vec{X: p.X * scale, Y: p.Y * scale, Z: p.Z * scale}
}

// NormalizeAngle ensures an angle is within [0, 2*Pi)
// Revit often returns angles in radians, sometimes outside normal range
func NormalizeAngle(angleRad float64) float64 {
	twoPi := 2 * math.Pi
	angle := math.Mod(angleRad, twoPi)
	if angle < 0 {
		angle += twoPi
	}
	return angle
}

// EnsureNormalUp adjusts a normal vector if it's not pointing generally "up"
// Useful for floor/ceiling detection where Revit families might be flipped
func EnsureNormalUp(n v3.Vec) v3.Vec {
	if n.Z < 0 {
		return v3.Vec{X: -n.X, Y: -n.Y, Z: -n.Z}
	}
	return n
}
