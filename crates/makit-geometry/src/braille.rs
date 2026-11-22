//! A dependency-free braille dot grid.
//!
//! Each terminal cell holds a 2×4 block of dots encoded as a Unicode braille
//! character (U+2800–U+28FF). The grid uses **y-up** coordinates: dot `(0, 0)`
//! is the bottom-left dot of the bottom-left cell.
//!
//! Output framing matches the old rsille canvas so existing drawings keep
//! their layout: the x range always includes the origin (drawings stay
//! left-anchored), while the y range is cropped to the content, aligned down
//! to a cell boundary.

use std::collections::BTreeSet;

use crate::plot::Plotter;

/// Base code point for braille patterns.
const BRAILLE_BASE: u32 = 0x2800;

/// Bit for the dot at `(column, row_from_top)` within a 2×4 cell.
const DOT_BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// A sparse grid of braille dots that implements [`Plotter`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrailleGrid {
    dots: BTreeSet<(i64, i64)>,
}

impl BrailleGrid {
    /// Create an empty grid.
    pub fn new() -> Self {
        Self::default()
    }

    /// True when no dots are set.
    pub fn is_empty(&self) -> bool {
        self.dots.is_empty()
    }

    /// Number of distinct dots set.
    pub fn dot_count(&self) -> usize {
        self.dots.len()
    }

    /// Whether the dot at integer coordinates `(x, y)` is set.
    pub fn is_set(&self, x: i64, y: i64) -> bool {
        self.dots.contains(&(x, y))
    }

    /// Output size in terminal cells as `(columns, rows)`; `(0, 0)` when empty.
    pub fn size(&self) -> (usize, usize) {
        match self.cell_extent() {
            Some((c0, c1, r0, r1)) => ((c1 - c0 + 1) as usize, (r1 - r0 + 1) as usize),
            None => (0, 0),
        }
    }

    /// Render the grid as lines of braille characters, top row first.
    pub fn to_lines(&self) -> Vec<String> {
        let Some((c0, c1, r0, r1)) = self.cell_extent() else {
            return Vec::new();
        };
        let cols = (c1 - c0 + 1) as usize;
        let rows = (r1 - r0 + 1) as usize;
        let mut cells = vec![vec![0u8; cols]; rows];

        for &(x, y) in &self.dots {
            let col = (x.div_euclid(2) - c0) as usize;
            // Row 0 of `cells` is the top of the output.
            let row = (r1 - y.div_euclid(4)) as usize;
            let dx = x.rem_euclid(2) as usize;
            let dy_from_top = 3 - y.rem_euclid(4) as usize;
            cells[row][col] |= DOT_BITS[dx][dy_from_top];
        }

        cells
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|bits| char::from_u32(BRAILLE_BASE + u32::from(bits)).unwrap_or(' '))
                    .collect()
            })
            .collect()
    }

    /// Print the grid to stdout.
    pub fn print(&self) {
        for line in self.to_lines() {
            println!("{line}");
        }
    }

    /// Inclusive cell extent `(col_min, col_max, row_min, row_max)` in cell
    /// coordinates (rows counted upward), or `None` when empty.
    fn cell_extent(&self) -> Option<(i64, i64, i64, i64)> {
        let mut iter = self.dots.iter();
        let &(x, y) = iter.next()?;
        let (mut x0, mut x1, mut y0, mut y1) = (x, x, y, y);
        for &(x, y) in iter {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        // x always includes the origin so drawings stay left-anchored.
        let x0 = x0.min(0);
        Some((
            x0.div_euclid(2),
            x1.div_euclid(2),
            y0.div_euclid(4),
            y1.div_euclid(4),
        ))
    }

    fn set_dot(&mut self, x: i64, y: i64) {
        self.dots.insert((x, y));
    }
}

/// Round a coordinate to the nearest dot index.
fn to_dot(v: f64) -> i64 {
    v.round() as i64
}

impl Plotter for BrailleGrid {
    fn set(&mut self, x: f64, y: f64) {
        if x.is_finite() && y.is_finite() {
            self.set_dot(to_dot(x), to_dot(y));
        }
    }

    /// Bresenham line between the rounded endpoints (inclusive).
    fn line(&mut self, a: (f64, f64), b: (f64, f64)) {
        if ![a.0, a.1, b.0, b.1].iter().all(|v| v.is_finite()) {
            return;
        }
        let (mut x, mut y) = (to_dot(a.0), to_dot(a.1));
        let (x1, y1) = (to_dot(b.0), to_dot(b.1));
        let dx = (x1 - x).abs();
        let dy = -(y1 - y).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let sy = if y < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set_dot(x, y);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(bits: u32) -> char {
        char::from_u32(BRAILLE_BASE + bits).unwrap()
    }

    #[test]
    fn empty_grid_renders_nothing() {
        let g = BrailleGrid::new();
        assert!(g.is_empty());
        assert_eq!(g.size(), (0, 0));
        assert!(g.to_lines().is_empty());
    }

    #[test]
    fn origin_dot_is_bottom_left_of_cell() {
        let mut g = BrailleGrid::new();
        g.set(0.0, 0.0);
        assert_eq!(g.to_lines(), vec!["⡀".to_string()]);
    }

    #[test]
    fn full_cell_sets_every_bit() {
        let mut g = BrailleGrid::new();
        for x in 0..2 {
            for y in 0..4 {
                g.set(x as f64, y as f64);
            }
        }
        assert_eq!(g.to_lines(), vec![ch(0xFF).to_string()]);
    }

    #[test]
    fn bit_order_follows_unicode_braille_layout() {
        // Top-left dot (x=0, y=3) is dot 1 (0x01); top-right (x=1, y=3) is dot 4 (0x08).
        let mut g = BrailleGrid::new();
        g.set(0.0, 3.0);
        assert_eq!(g.to_lines(), vec![ch(0x01).to_string()]);

        let mut g = BrailleGrid::new();
        g.set(1.0, 3.0);
        g.set(0.0, 0.0); // keep the cell anchored at the origin row
        assert_eq!(g.to_lines(), vec![ch(0x08 | 0x40).to_string()]);

        let mut g = BrailleGrid::new();
        g.set(1.0, 0.0);
        assert_eq!(g.to_lines(), vec![ch(0x80).to_string()]);
    }

    #[test]
    fn higher_y_renders_on_earlier_lines() {
        let mut g = BrailleGrid::new();
        g.set(0.0, 0.0);
        g.set(0.0, 4.0);
        // Two rows: the dot at y=4 is the bottom-left of the upper cell.
        assert_eq!(g.to_lines(), vec!["⡀".to_string(), "⡀".to_string()]);
    }

    #[test]
    fn x_range_includes_origin() {
        let mut g = BrailleGrid::new();
        g.set(4.0, 0.0);
        assert_eq!(g.size(), (3, 1));
        assert_eq!(g.to_lines(), vec![format!("{0}{0}⡀", ch(0))]);
    }

    #[test]
    fn y_range_is_cropped_and_cell_aligned() {
        let mut g = BrailleGrid::new();
        g.set(0.0, 21.0); // cell rows start at y=20
        assert_eq!(g.size(), (1, 1));
        assert_eq!(g.to_lines(), vec![ch(0x04).to_string()]);
    }

    #[test]
    fn negative_coordinates_are_supported() {
        let mut g = BrailleGrid::new();
        g.set(-1.0, -1.0);
        g.set(0.0, 0.0);
        // Cells stay aligned to the origin: x=-1 lives in the cell left of x=0.
        assert_eq!(g.size(), (2, 2));
        assert!(g.is_set(-1, -1));
    }

    #[test]
    fn fractional_coordinates_round_to_nearest_dot() {
        let mut g = BrailleGrid::new();
        g.set(1.6, 2.4);
        assert!(g.is_set(2, 2));
        assert_eq!(g.dot_count(), 1);
    }

    #[test]
    fn horizontal_line_sets_every_dot() {
        let mut g = BrailleGrid::new();
        g.line((0.0, 0.0), (5.0, 0.0));
        assert_eq!(g.dot_count(), 6);
        assert!((0..=5).all(|x| g.is_set(x, 0)));
    }

    #[test]
    fn diagonal_line_is_continuous() {
        let mut g = BrailleGrid::new();
        g.line((0.0, 0.0), (7.0, 3.0));
        assert!(g.is_set(0, 0));
        assert!(g.is_set(7, 3));
        // Bresenham: exactly one dot per x step for a shallow line.
        assert_eq!(g.dot_count(), 8);
    }

    #[test]
    fn reversed_line_matches_forward_line_endpoints() {
        let mut g = BrailleGrid::new();
        g.line((7.0, 3.0), (0.0, 0.0));
        assert!(g.is_set(0, 0));
        assert!(g.is_set(7, 3));
        assert_eq!(g.dot_count(), 8);
    }

    #[test]
    fn duplicate_dots_are_counted_once() {
        let mut g = BrailleGrid::new();
        g.set(1.0, 1.0);
        g.set(1.2, 0.9);
        assert_eq!(g.dot_count(), 1);
    }
}
