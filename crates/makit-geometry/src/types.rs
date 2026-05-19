//! Core 2D geometry types.

/// A 2D point with floating-point coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A line segment defined by start and end points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

impl Line {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    /// Returns the length of this line segment.
    pub fn length(&self) -> f64 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

/// A room with walls and metadata.
#[derive(Debug, Clone)]
pub struct Room {
    pub name: String,
    pub walls: Vec<Line>,
    pub area: f64,
}

/// A floor plan containing rooms and walls.
#[derive(Debug, Clone)]
pub struct Floor {
    pub name: String,
    pub rooms: Vec<Room>,
    pub walls: Vec<Line>,
}

/// A face for isometric rendering (wall or window).
#[derive(Debug, Clone)]
pub struct Face {
    pub points: Vec<Point>,
    pub face_type: FaceType,
}

/// Type of face for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceType {
    Wall,
    Window,
}

/// Directional statistics for wall orientation analysis.
#[derive(Debug, Clone)]
pub struct DirectionStats {
    pub walls: usize,
    pub windows: usize,
    pub wall_area: f64,
    pub window_area: f64,
    pub wwr: f64,
    pub faces: Vec<Face>,
}

/// Calculate the bounding box for a set of lines.
pub fn get_bounds(lines: &[Line]) -> Rectangle {
    if lines.is_empty() {
        return Rectangle::new(0.0, 0.0, 1.0, 1.0);
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for line in lines {
        for p in [line.start, line.end] {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }

    Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Scale a world-space point to canvas pixel coordinates.
pub fn scale_point(
    p: Point,
    bounds: &Rectangle,
    canvas_width: i32,
    canvas_height: i32,
) -> (f64, f64) {
    let padding = 2.0;
    let avail_width = (canvas_width * 2) as f64 - padding * 2.0;
    let avail_height = (canvas_height * 4) as f64 - padding * 2.0;

    let scale_x = avail_width / bounds.width;
    let scale_y = avail_height / bounds.height;
    let scale = scale_x.min(scale_y);

    let offset_x = padding + (avail_width - bounds.width * scale) / 2.0;
    let offset_y = padding + (avail_height - bounds.height * scale) / 2.0;

    let x = (p.x - bounds.x) * scale + offset_x;
    let y = (p.y - bounds.y) * scale + offset_y;

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_new() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 4.0);
    }

    #[test]
    fn test_line_length() {
        let l = Line::new(Point::new(0.0, 0.0), Point::new(3.0, 4.0));
        assert!((l.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_rectangle_area() {
        let r = Rectangle::new(0.0, 0.0, 10.0, 5.0);
        assert_eq!(r.area(), 50.0);
    }

    #[test]
    fn test_get_bounds_empty() {
        let bounds = get_bounds(&[]);
        assert_eq!(bounds.width, 1.0);
        assert_eq!(bounds.height, 1.0);
    }

    #[test]
    fn test_get_bounds() {
        let lines = vec![
            Line::new(Point::new(0.0, 0.0), Point::new(10.0, 5.0)),
            Line::new(Point::new(-3.0, -2.0), Point::new(7.0, 8.0)),
        ];
        let bounds = get_bounds(&lines);
        assert_eq!(bounds.x, -3.0);
        assert_eq!(bounds.y, -2.0);
        assert_eq!(bounds.width, 13.0);
        assert_eq!(bounds.height, 10.0);
    }

    #[test]
    fn test_scale_point_centered() {
        let bounds = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let (x, y) = scale_point(Point::new(50.0, 50.0), &bounds, 40, 20);
        // Should be roughly centered
        assert!(x > 10.0 && x < 70.0);
        assert!(y > 10.0 && y < 70.0);
    }
}
