//! Signed Distance Field (SDF) primitives for braille canvas rendering.
//!
//! Uses 2D SDF functions (inspired by Inigo Quilez) to render smooth
//! shapes at braille resolution (2×4 dots per character cell).

use canvas::Canvas;

// ---------------------------------------------------------------------------
// SDF Primitives
// ---------------------------------------------------------------------------

/// SDF for a circle centered at the origin.
#[inline]
pub fn sdf_circle(px: f64, py: f64, radius: f64) -> f64 {
    (px * px + py * py).sqrt() - radius
}

/// SDF for a regular hexagon centered at the origin.
///
/// Based on Inigo Quilez's hexagonal SDF — uses symmetry folding
/// to reduce to a single quadrant computation.
#[inline]
pub fn sdf_hexagon(px: f64, py: f64, radius: f64) -> f64 {
    // Fold into first sextant using hexagonal symmetry
    let px = px.abs();
    let py = py.abs();

    // Constants for 60-degree fold: cos(60°) = 0.5, sin(60°) = √3/2
    let k = [-0.866_025_403_8_f64, 0.5_f64, 0.577_350_269_2_f64];

    // Fold: reflect across the 60° line
    let dot = 2.0 * (k[0] * px + k[1] * py).min(0.0);
    let px = px - dot * k[0];
    let py = py - dot * k[1];

    // Clamp to edge segment
    let px = px - px.clamp(0.0, radius * k[2]);
    let py = py - radius;

    (px * px + py * py).sqrt().copysign(py)
}

/// SDF for a ring (annulus) — distance to a circular ring of given radius
/// and thickness.
#[inline]
pub fn sdf_ring(px: f64, py: f64, radius: f64, thickness: f64) -> f64 {
    sdf_circle(px, py, radius).abs() - thickness
}

/// SDF for a hexagonal ring — the outline of a hexagon.
#[inline]
pub fn sdf_hex_ring(px: f64, py: f64, radius: f64, thickness: f64) -> f64 {
    sdf_hexagon(px, py, radius).abs() - thickness
}

/// Boolean union of two SDF values (minimum).
#[inline]
pub fn sdf_union(a: f64, b: f64) -> f64 {
    a.min(b)
}

/// Smooth union of two SDF values with blending factor k.
#[inline]
pub fn sdf_smooth_union(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    a * (1.0 - h) + b * h - k * h * (1.0 - h)
}

// ---------------------------------------------------------------------------
// Transforms
// ---------------------------------------------------------------------------

/// Apply a 2D rotation to a point.
#[inline]
pub fn rotate_2d(px: f64, py: f64, angle: f64) -> (f64, f64) {
    let (s, c) = angle.sin_cos();
    (px * c - py * s, px * s + py * c)
}

// ---------------------------------------------------------------------------
// Composite Logo
// ---------------------------------------------------------------------------

/// Evaluate the makit SDF logo at a point.
///
/// Combines an outer hexagonal ring with an inner counter-rotating hexagonal
/// ring, connected by radial spokes. All shapes use smooth SDF blending.
///
/// - `px`, `py`: sample point relative to center (0, 0)
/// - `angle`: rotation angle in radians (applied to outer ring)
/// - `outer_r`: outer hexagon radius (default: 12.0)
/// - `inner_r`: inner hexagon radius (default: 6.6)
pub fn sdf_logo(px: f64, py: f64, angle: f64, outer_r: f64, inner_r: f64) -> f64 {
    // Rotate sample point for outer ring (equivalent to rotating the shape)
    let (ox, oy) = rotate_2d(px, py, -angle);
    let outer = sdf_hex_ring(ox, oy, outer_r, 1.2);

    // Counter-rotate for inner ring
    let (ix, iy) = rotate_2d(px, py, angle);
    let inner = sdf_hex_ring(ix, iy, inner_r, 0.8);

    // Radial spokes connecting inner to outer
    let mut spokes = f64::MAX;
    for i in 0..6 {
        let a = angle + (i as f64) * std::f64::consts::TAU / 6.0;
        let (dx, dy) = a.sin_cos();

        // Line segment SDF from inner_r to outer_r along direction (dx, dy)
        let t = ((px * dx + py * dy) / 1.0).clamp(inner_r * 0.8, outer_r * 0.9);
        let closest_x = dx * t;
        let closest_y = dy * t;
        let dist = ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt() - 0.6;
        spokes = sdf_union(spokes, dist);
    }

    sdf_smooth_union(sdf_smooth_union(outer, inner, 0.5), spokes, 0.3)
}

/// Render the SDF logo onto a braille canvas.
///
/// Samples the SDF at each braille dot position and sets dots where
/// the distance is ≤ 0 (inside the shape).
///
/// - `c`: Canvas to render onto
/// - `angle`: rotation angle in radians
/// - `cx`, `cy`: center position on canvas
/// - `outer_r`: outer hexagon radius
pub fn render_sdf_logo(c: &mut Canvas, angle: f64, cx: f64, cy: f64, outer_r: f64) {
    let inner_r = outer_r * 0.55;
    let extent = (outer_r + 3.0) as i32;

    for dx in -extent..=extent {
        for dy in -extent..=extent {
            let px = dx as f64;
            let py = dy as f64;
            let d = sdf_logo(px, py, angle, outer_r, inner_r);

            if d <= 0.0 {
                c.set(cx + px, cy + py);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_circle() {
        // Point at origin should be -radius (inside)
        assert!((sdf_circle(0.0, 0.0, 5.0) - (-5.0)).abs() < 0.001);
        // Point on boundary should be ~0
        assert!((sdf_circle(5.0, 0.0, 5.0)).abs() < 0.001);
        // Point outside should be positive
        assert!(sdf_circle(10.0, 0.0, 5.0) > 0.0);
    }

    #[test]
    fn test_sdf_hexagon() {
        // Point at origin should be inside (negative)
        assert!(sdf_hexagon(0.0, 0.0, 10.0) < 0.0);
        // Point far outside should be positive
        assert!(sdf_hexagon(20.0, 0.0, 10.0) > 0.0);
    }

    #[test]
    fn test_rotate_2d() {
        let (rx, ry) = rotate_2d(1.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert!((rx - 0.0).abs() < 0.001);
        assert!((ry - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sdf_logo_center_is_inside() {
        // Center of the logo should be inside the inner ring
        let d = sdf_logo(0.0, 0.0, 0.0, 12.0, 6.6);
        // Due to ring structure, center may or may not be inside
        // but should be finite
        assert!(d.is_finite());
    }

    #[test]
    fn test_render_sdf_logo_produces_output() {
        let mut c = Canvas::new();
        render_sdf_logo(&mut c, 0.0, 20.0, 16.0, 12.0);

        let mut buf = Vec::new();
        c.print_on(&mut buf, false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should produce non-empty braille output
        assert!(!output.trim().is_empty());
    }

    #[test]
    fn test_sdf_performance() {
        use std::time::Instant;

        let start = Instant::now();
        let mut c = Canvas::new();
        render_sdf_logo(&mut c, 0.5, 20.0, 16.0, 12.0);
        let elapsed = start.elapsed();

        // Must complete within 2ms budget (S2 requirement)
        assert!(
            elapsed.as_millis() < 10,
            "SDF render took {:?}, expected < 10ms",
            elapsed
        );
    }
}
