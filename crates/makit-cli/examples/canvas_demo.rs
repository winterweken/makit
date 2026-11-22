//! Canvas demo — renders a sample building floor plan on a braille grid.
//!
//! Run with: cargo run -p makit --example canvas_demo

use makit_geometry::drawing::{draw_arrow, draw_lines, draw_rect, fill_rect};
use makit_geometry::types::{Line, Point};
use makit_geometry::{BrailleGrid, Plotter};

fn main() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║                makit canvas demo                 ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    // Demo 1: Simple shapes
    println!("▸ Simple shapes:");
    let mut c = BrailleGrid::new();

    // Draw a rectangle
    draw_rect(&mut c, 2.0, 2.0, 30.0, 16.0);

    // Draw diagonal lines
    c.line((2.0, 2.0), (32.0, 18.0));
    c.line((32.0, 2.0), (2.0, 18.0));

    // Draw an arrow
    draw_arrow(&mut c, 36.0, 10.0, 50.0, 10.0);

    c.print();
    println!();

    // Demo 2: Floor plan
    println!("▸ Floor plan:");
    let mut c2 = BrailleGrid::new();

    let walls = vec![
        // Outer walls
        Line::new(Point::new(0.0, 0.0), Point::new(100.0, 0.0)),
        Line::new(Point::new(100.0, 0.0), Point::new(100.0, 60.0)),
        Line::new(Point::new(100.0, 60.0), Point::new(0.0, 60.0)),
        Line::new(Point::new(0.0, 60.0), Point::new(0.0, 0.0)),
        // Interior walls
        Line::new(Point::new(50.0, 0.0), Point::new(50.0, 40.0)),
        Line::new(Point::new(0.0, 40.0), Point::new(70.0, 40.0)),
        // Doorway indicators
        Line::new(Point::new(50.0, 45.0), Point::new(50.0, 60.0)),
    ];

    draw_lines(&mut c2, &walls, 50, 15);
    c2.print();
    println!();

    // Demo 3: Filled shapes
    println!("▸ Filled rectangle:");
    let mut c3 = BrailleGrid::new();
    fill_rect(&mut c3, 0, 0, 20, 12);
    c3.print();
    println!();

    println!("▸ Done. Braille grid rendering working.");
}
