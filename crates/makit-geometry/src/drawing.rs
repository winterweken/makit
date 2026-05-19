//! Drawing utilities using rsille's braille canvas.
//!
//! Provides functions for drawing geometry primitives on a braille canvas,
//! including lines, walls (thick lines as filled polygons), and scanline
//! polygon fill.

use canvas::Canvas;
use crate::types::{Line, Point, Rectangle, get_bounds, scale_point};

/// Draw a set of lines on a canvas, auto-scaled to fit.
pub fn draw_lines(
    canvas: &mut Canvas,
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
pub fn draw_wall(
    canvas: &mut Canvas,
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
pub fn draw_thick_line(
    canvas: &mut Canvas,
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
pub fn fill_polygon(canvas: &mut Canvas, points: &[(i32, i32)]) {
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
pub fn draw_rect(canvas: &mut Canvas, x: f64, y: f64, w: f64, h: f64) {
    canvas.line((x, y), (x + w, y));
    canvas.line((x + w, y), (x + w, y + h));
    canvas.line((x + w, y + h), (x, y + h));
    canvas.line((x, y + h), (x, y));
}

/// Fill a rectangle on the canvas.
pub fn fill_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    for dy in 0..=h {
        for dx in 0..=w {
            canvas.set((x + dx) as f64, (y + dy) as f64);
        }
    }
}

/// Draw an arrow from (x0,y0) to (x1,y1).
pub fn draw_arrow(canvas: &mut Canvas, x0: f64, y0: f64, x1: f64, y1: f64) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_polygon_triangle() {
        let mut c = Canvas::new();
        let points = [(0, 0), (10, 0), (5, 10)];
        fill_polygon(&mut c, &points);
        // Canvas should have some pixels set - just verify no panic
        let (w, h) = c.get_size();
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn test_draw_lines_empty() {
        let mut c = Canvas::new();
        draw_lines(&mut c, &[], 40, 20);
        // Should not panic
    }

    #[test]
    fn test_draw_rect() {
        let mut c = Canvas::new();
        draw_rect(&mut c, 0.0, 0.0, 10.0, 5.0);
        let (w, h) = c.get_size();
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn test_draw_arrow() {
        let mut c = Canvas::new();
        draw_arrow(&mut c, 0.0, 0.0, 20.0, 10.0);
        let (w, h) = c.get_size();
        assert!(w > 0);
        assert!(h > 0);
    }
}
