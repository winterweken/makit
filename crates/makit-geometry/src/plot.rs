//! Backend-agnostic plotting surface.
//!
//! Drawing helpers in this crate render onto any [`Plotter`], so the same
//! geometry code can target a plain braille grid ([`crate::BrailleGrid`]),
//! a ratatui canvas (via [`Recording`]), or anything else that can plot
//! dots and lines.
//!
//! Coordinates are in "dot" units with **y pointing up** — one unit equals
//! one braille dot when drawn at 1:1 scale.

use crate::types::Rectangle;

/// A surface that can plot dots, line segments and text labels.
pub trait Plotter {
    /// Plot a single dot.
    fn set(&mut self, x: f64, y: f64);

    /// Plot a straight line segment from `a` to `b`.
    fn line(&mut self, a: (f64, f64), b: (f64, f64));

    /// Place a text label anchored at `(x, y)`.
    ///
    /// Surfaces that cannot render text ignore labels (the default).
    fn label(&mut self, _x: f64, _y: f64, _text: &str) {}
}

/// A [`Plotter`] that records every primitive instead of rasterising it.
///
/// Renderers replay a recording onto their own canvas — for example the TUI
/// scales it to fit the preview pane before drawing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recording {
    /// Line segments as `(start, end)` pairs.
    pub segments: Vec<((f64, f64), (f64, f64))>,
    /// Individual dots.
    pub points: Vec<(f64, f64)>,
    /// Text labels as `(x, y, text)`.
    pub labels: Vec<(f64, f64, String)>,
}

impl Recording {
    /// Create an empty recording.
    pub fn new() -> Self {
        Self::default()
    }

    /// True when nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.points.is_empty() && self.labels.is_empty()
    }

    /// Axis-aligned bounds of everything recorded (label anchors included),
    /// or `None` for an empty recording.
    pub fn bounds(&self) -> Option<Rectangle> {
        let coords = self
            .segments
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .chain(self.points.iter().copied())
            .chain(self.labels.iter().map(|(x, y, _)| (*x, *y)));

        let mut acc: Option<(f64, f64, f64, f64)> = None;
        for (x, y) in coords {
            acc = Some(match acc {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
        acc.map(|(x0, y0, x1, y1)| Rectangle::new(x0, y0, x1 - x0, y1 - y0))
    }
}

impl Plotter for Recording {
    fn set(&mut self, x: f64, y: f64) {
        self.points.push((x, y));
    }

    fn line(&mut self, a: (f64, f64), b: (f64, f64)) {
        self.segments.push((a, b));
    }

    fn label(&mut self, x: f64, y: f64, text: &str) {
        self.labels.push((x, y, text.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_recording_has_no_bounds() {
        let r = Recording::new();
        assert!(r.is_empty());
        assert_eq!(r.bounds(), None);
    }

    #[test]
    fn recording_captures_all_primitive_kinds() {
        let mut r = Recording::new();
        r.set(1.0, 2.0);
        r.line((0.0, 0.0), (3.0, 4.0));
        r.label(5.0, -1.0, "N");
        assert_eq!(r.points, vec![(1.0, 2.0)]);
        assert_eq!(r.segments, vec![((0.0, 0.0), (3.0, 4.0))]);
        assert_eq!(r.labels, vec![(5.0, -1.0, "N".to_string())]);
    }

    #[test]
    fn bounds_span_segments_points_and_labels() {
        let mut r = Recording::new();
        r.line((2.0, 3.0), (10.0, 8.0));
        r.set(-4.0, 5.0);
        r.label(6.0, -2.0, "x");
        let b = r.bounds().unwrap();
        assert_eq!(b, Rectangle::new(-4.0, -2.0, 14.0, 10.0));
    }

    #[test]
    fn default_label_is_a_no_op_for_simple_plotters() {
        struct Counter(usize);
        impl Plotter for Counter {
            fn set(&mut self, _x: f64, _y: f64) {
                self.0 += 1;
            }
            fn line(&mut self, _a: (f64, f64), _b: (f64, f64)) {
                self.0 += 1;
            }
        }
        let mut c = Counter(0);
        c.label(0.0, 0.0, "ignored");
        assert_eq!(c.0, 0);
    }
}
