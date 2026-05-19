//! Render braille art for the README — run with:
//! cargo run -p makit --example readme_render

use canvas::Canvas;
use makit_geometry::drawing::{draw_arrow, draw_isometric_box, draw_rect, fill_rect};

fn main() {
    // --- Section 1: Isometric building ---
    println!("▸ Isometric building:");
    let mut c = Canvas::new();
    let ox = 20.0;
    let oy = 28.0;
    draw_isometric_box(&mut c, ox, oy, 10.0, 6.0, 8.0);
    draw_isometric_box(&mut c, ox, oy, 10.0, 3.0, 8.0);
    draw_isometric_box(&mut c, ox, oy, 10.0, 7.0, 8.0);
    c.print();
    println!();

    // --- Section 2: Floor plan ---
    println!("▸ Floor plan:");
    let mut c2 = Canvas::new();
    draw_rect(&mut c2, 2.0, 2.0, 36.0, 28.0);
    c2.line((20.0, 2.0), (20.0, 22.0));
    c2.line((2.0, 18.0), (36.0, 18.0));
    c2.line((20.0, 24.0), (20.0, 30.0));
    draw_arrow(&mut c2, 20.0, 32.0, 20.0, 36.0);
    draw_arrow(&mut c2, 40.0, 16.0, 44.0, 16.0);
    c2.print();
    println!();

    // --- Section 3: Energy bar chart ---
    println!("▸ Energy (heating demand):");
    let mut c3 = Canvas::new();
    let months = [28, 24, 18, 10, 5, 2, 1, 2, 6, 14, 22, 26];
    for (i, height) in months.iter().enumerate() {
        let x = (i as i32) * 3 + 2;
        fill_rect(&mut c3, x, 0, 2, *height);
    }
    c3.line((0.0, 0.0), (40.0, 0.0));
    c3.line((0.0, 0.0), (0.0, 32.0));
    c3.print();
    println!("   J  F  M  A  M  J  J  A  S  O  N  D");
}
