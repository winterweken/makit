//! Drawing utilities for any [`Plotter`] surface.
//!
//! Provides functions for drawing geometry primitives on a braille-resolution
//! surface, including lines, walls (thick lines as filled polygons), and
//! scanline polygon fill.

use crate::plot::Plotter;
use crate::types::{get_bounds, scale_point, Line, Point, Rectangle};

/// Draw a set of lines on a canvas, auto-scaled to fit.
pub fn draw_lines<P: Plotter + ?Sized>(
    canvas: &mut P,
    lines: &[Line],
    canvas_width: i32,
    canvas_height: i32,
) {
    if lines.is_empty() {
        return;
    }

    let bounds = get_bounds(lines);

    for line in lines {
        let (x0, y0) = scale_point(line.start, &bounds, canvas_width, canvas_height);
        let (x1, y1) = scale_point(line.end, &bounds, canvas_width, canvas_height);
        canvas.line((x0, y0), (x1, y1));
    }
}

/// Draw a wall as a filled polygon with thickness.
pub fn draw_wall<P: Plotter + ?Sized>(
    canvas: &mut P,
    line: &Line,
    thickness: f64,
    bounds: &Rectangle,
    canvas_width: i32,
    canvas_height: i32,
) {
    let dx = line.end.x - line.start.x;
    let dy = line.end.y - line.start.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length == 0.0 {
        return;
    }

    let nx = -dy / length * (thickness / 2.0);
    let ny = dx / length * (thickness / 2.0);

    // 4 corners of the wall rectangle
    let p1 = Point::new(line.start.x + nx, line.start.y + ny);
    let p2 = Point::new(line.end.x + nx, line.end.y + ny);
    let p3 = Point::new(line.end.x - nx, line.end.y - ny);
    let p4 = Point::new(line.start.x - nx, line.start.y - ny);

    let (sx1, sy1) = scale_point(p1, bounds, canvas_width, canvas_height);
    let (sx2, sy2) = scale_point(p2, bounds, canvas_width, canvas_height);
    let (sx3, sy3) = scale_point(p3, bounds, canvas_width, canvas_height);
    let (sx4, sy4) = scale_point(p4, bounds, canvas_width, canvas_height);

    let points = [
        (sx1 as i32, sy1 as i32),
        (sx2 as i32, sy2 as i32),
        (sx3 as i32, sy3 as i32),
        (sx4 as i32, sy4 as i32),
    ];

    fill_polygon(canvas, &points);
}

/// Draw a line with thickness — uses wall polygon for thick, single line for thin.
pub fn draw_thick_line<P: Plotter + ?Sized>(
    canvas: &mut P,
    line: &Line,
    thickness: f64,
    bounds: &Rectangle,
    canvas_width: i32,
    canvas_height: i32,
) {
    if thickness <= 0.5 {
        let (x0, y0) = scale_point(line.start, bounds, canvas_width, canvas_height);
        let (x1, y1) = scale_point(line.end, bounds, canvas_width, canvas_height);
        canvas.line((x0, y0), (x1, y1));
    } else {
        draw_wall(canvas, line, thickness, bounds, canvas_width, canvas_height);
    }
}

/// Fill a polygon using scanline algorithm.
///
/// Points should be in canvas pixel coordinates (integer).
pub fn fill_polygon<P: Plotter + ?Sized>(canvas: &mut P, points: &[(i32, i32)]) {
    if points.len() < 3 {
        return;
    }

    let min_y = points.iter().map(|p| p.1).min().unwrap();
    let max_y = points.iter().map(|p| p.1).max().unwrap();

    for y in min_y..=max_y {
        let mut nodes: Vec<i32> = Vec::new();
        let n = points.len();
        let mut j = n - 1;

        for i in 0..n {
            let (_, yi) = points[i];
            let (_, yj) = points[j];

            if (yi < y && yj >= y) || (yj < y && yi >= y) {
                let (xi, _) = points[i];
                let (xj, _) = points[j];
                let node_x = xi + (y - yi) * (xj - xi) / (yj - yi);
                nodes.push(node_x);
            }
            j = i;
        }

        nodes.sort();

        let mut i = 0;
        while i + 1 < nodes.len() {
            for x in nodes[i]..=nodes[i + 1] {
                canvas.set(x as f64, y as f64);
            }
            i += 2;
        }
    }
}

/// Draw a rectangle outline on the canvas.
pub fn draw_rect<P: Plotter + ?Sized>(canvas: &mut P, x: f64, y: f64, w: f64, h: f64) {
    canvas.line((x, y), (x + w, y));
    canvas.line((x + w, y), (x + w, y + h));
    canvas.line((x + w, y + h), (x, y + h));
    canvas.line((x, y + h), (x, y));
}

/// Fill a rectangle on the canvas.
pub fn fill_rect<P: Plotter + ?Sized>(canvas: &mut P, x: i32, y: i32, w: i32, h: i32) {
    for dy in 0..=h {
        for dx in 0..=w {
            canvas.set((x + dx) as f64, (y + dy) as f64);
        }
    }
}

/// Draw an arrow from (x0,y0) to (x1,y1).
pub fn draw_arrow<P: Plotter + ?Sized>(canvas: &mut P, x0: f64, y0: f64, x1: f64, y1: f64) {
    canvas.line((x0, y0), (x1, y1));

    let dx = x1 - x0;
    let dy = y1 - y0;
    let length = (dx * dx + dy * dy).sqrt();

    if length > 0.0 {
        let dx = dx / length;
        let dy = dy / length;

        // Perpendicular
        let px = -dy;
        let py = dx;

        let head_len = 3.0;

        // Wing 1
        let wx1 = x1 - dx * head_len + px * head_len * 0.6;
        let wy1 = y1 - dy * head_len + py * head_len * 0.6;

        // Wing 2
        let wx2 = x1 - dx * head_len - px * head_len * 0.6;
        let wy2 = y1 - dy * head_len - py * head_len * 0.6;

        canvas.line((x1, y1), (wx1, wy1));
        canvas.line((x1, y1), (wx2, wy2));
    }
}

/// cos 30°, the horizontal spread of a 30° isometric.
const COS30: f64 = 0.866_025_403_8;

/// Project a 3D point to 2D screen coordinates using 30° dimetric
/// isometric projection.
///
/// - `x`, `y`, `z`: world coordinates (y is up)
/// - Returns `(screen_x, screen_y)` in canvas coordinates, y up on screen
pub fn isometric_project(x: f64, y: f64, z: f64) -> (f64, f64) {
    // Standard 30° isometric: cos(30°) ≈ 0.866, sin(30°) = 0.5
    let sin30 = 0.5_f64;

    let sx = (x - z) * COS30;
    let sy = (x + z) * sin30 + y;
    (sx, sy)
}

/// Project a point in project coordinates (x east, y north, z up) into a
/// 30° isometric seen from the south-west: east runs right and up, north
/// left and up, heights straight up. Screen y points up, one unit per metre.
pub fn isometric_z_up(x: f64, y: f64, z: f64) -> (f64, f64) {
    ((x - y) * COS30, (x + y) * 0.5 + z)
}

/// Draw an isometric wireframe box on the canvas.
///
/// - `c`: surface to render onto
/// - `ox`, `oy`: screen-space offset for positioning
/// - `w`, `h`, `d`: box dimensions in world units (width, height, depth)
pub fn draw_isometric_box<P: Plotter + ?Sized>(
    c: &mut P,
    ox: f64,
    oy: f64,
    w: f64,
    h: f64,
    d: f64,
) {
    // 8 corners of the box
    let corners = [
        (0.0, 0.0, 0.0),
        (w, 0.0, 0.0),
        (w, 0.0, d),
        (0.0, 0.0, d), // bottom
        (0.0, h, 0.0),
        (w, h, 0.0),
        (w, h, d),
        (0.0, h, d), // top
    ];

    let projected: Vec<(f64, f64)> = corners
        .iter()
        .map(|&(x, y, z)| {
            let (sx, sy) = isometric_project(x, y, z);
            (ox + sx, oy + sy)
        })
        .collect();

    // 12 edges of a box
    let edges = [
        // Bottom face
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        // Top face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        // Vertical edges
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];

    for &(a, b) in &edges {
        c.line(projected[a], projected[b]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::braille::BrailleGrid;

    #[test]
    fn test_fill_polygon_triangle() {
        let mut c = BrailleGrid::new();
        let points = [(0, 0), (10, 0), (5, 10)];
        fill_polygon(&mut c, &points);
        // Grid should have some dots set
        assert!(!c.is_empty());
    }

    #[test]
    fn test_draw_lines_empty() {
        let mut c = BrailleGrid::new();
        draw_lines(&mut c, &[], 40, 20);
        // Should not panic
    }

    #[test]
    fn test_draw_rect() {
        let mut c = BrailleGrid::new();
        draw_rect(&mut c, 0.0, 0.0, 10.0, 5.0);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_draw_arrow() {
        let mut c = BrailleGrid::new();
        draw_arrow(&mut c, 0.0, 0.0, 20.0, 10.0);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_isometric_project() {
        // Origin maps to origin
        let (sx, sy) = isometric_project(0.0, 0.0, 0.0);
        assert!((sx).abs() < 0.001);
        assert!((sy).abs() < 0.001);

        // Moving along X should move right and slightly down
        let (sx, _sy) = isometric_project(10.0, 0.0, 0.0);
        assert!(sx > 0.0);

        // Moving up (Y) moves up on a y-up screen
        let (_sx, sy) = isometric_project(0.0, 10.0, 0.0);
        assert!(sy > 0.0);
    }

    #[test]
    fn z_up_isometric_puts_east_right_north_left_and_heights_straight_up() {
        let (ex, ey) = isometric_z_up(10.0, 0.0, 0.0);
        assert!(ex > 0.0 && ey > 0.0);
        let (nx, ny) = isometric_z_up(0.0, 10.0, 0.0);
        assert!(nx < 0.0 && ny > 0.0);
        assert_eq!(isometric_z_up(0.0, 0.0, 3.0), (0.0, 3.0));
        // A square in plan projects to a 2:1 diamond.
        assert!((ex - 8.660_254_038).abs() < 1e-9 && (ey - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_draw_isometric_box() {
        let mut c = BrailleGrid::new();
        draw_isometric_box(&mut c, 20.0, 20.0, 10.0, 8.0, 6.0);
        assert!(!c.is_empty());
    }
    #[test]
    fn test_draw_wall() {
        let mut c = BrailleGrid::new();
        let line = Line::new(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let bounds = Rectangle::new(-5.0, -5.0, 20.0, 10.0);
        draw_wall(&mut c, &line, 2.0, &bounds, 40, 20);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_draw_thick_line() {
        let mut c = BrailleGrid::new();
        let line = Line::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let bounds = Rectangle::new(-5.0, -5.0, 20.0, 20.0);

        // Test thin line
        draw_thick_line(&mut c, &line, 0.2, &bounds, 40, 20);

        // Test thick wall
        draw_thick_line(&mut c, &line, 2.0, &bounds, 40, 20);
        assert!(!c.is_empty());
    }
}
