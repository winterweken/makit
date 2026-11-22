//! The makit symbol: a 2×2×2 kit of blocks with the front-top block lifted
//! out of its socket — the one being made.
//!
//! Three forms share the same idea:
//!
//! - [`segments`] / [`draw`]: the full symbol as a hidden-line isometric
//!   wireframe for the braille preview. The kit is drawn in [`KIT`]; the lifted
//!   block's faces use the theme's three colours (top amber, left cyan, right
//!   purple) and it hovers gently with the animation angle.
//! - [`header_icon`]: a two-cell "pixel cube" for the header bar.
//! - [`mini`]: a 4×2-cell hexagonal cube for places with a little more room.

use makit_geometry::Plotter;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::THEME;

/// Centre and radius of the symbol in preview dot units (y up). The symbol
/// always fits inside this circle's bounding square, whatever the angle.
pub const CENTER: (f64, f64) = (20.0, 16.0);
pub const RADIUS: f64 = 12.0;

/// Edge length of one block in dot units.
const UNIT: f64 = 5.6;
/// Resting lift of the hero block, in block heights, and how far it sways.
const LIFT_REST: f64 = 0.62;
const LIFT_SWAY: f64 = 0.13;

/// Kit colour: the seven placed blocks.
pub const KIT: Color = Color::Rgb(112, 118, 140);
/// Hero block faces.
pub const TOP: Color = THEME.accent;
pub const LEFT: Color = THEME.primary;
pub const RIGHT: Color = THEME.secondary;

/// Which part of the symbol a segment belongs to, for colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Kit,
    Top,
    Left,
    Right,
}

impl Tone {
    pub fn color(self) -> Color {
        match self {
            Tone::Kit => KIT,
            Tone::Top => TOP,
            Tone::Left => LEFT,
            Tone::Right => RIGHT,
        }
    }
}

type Pt = (f64, f64);
type Seg = (Pt, Pt);

/// How far the hero block is lifted at `angle` (radians, any value).
pub fn lift(angle: f64) -> f64 {
    LIFT_REST + LIFT_SWAY * angle.sin()
}

/// Isometric projection of block-grid coordinates into dot units (y up).
fn project(x: f64, y: f64, z: f64) -> Pt {
    let c = (std::f64::consts::PI / 6.0).cos();
    (
        CENTER.0 + (x - y) * c * UNIT,
        CENTER.1 + (z - 0.5 * (x + y)) * UNIT,
    )
}

/// Visible faces of every block, nearest first, with their tone.
fn faces(lift: f64) -> Vec<([Pt; 4], Tone)> {
    let mut blocks: Vec<(f64, f64, f64, bool)> = Vec::with_capacity(8);
    for bx in 0..2 {
        for by in 0..2 {
            for bz in 0..2 {
                let hero = (bx, by, bz) == (1, 1, 1);
                let z = f64::from(bz) + if hero { lift } else { 0.0 };
                blocks.push((f64::from(bx), f64::from(by), z, hero));
            }
        }
    }
    // Nearer blocks have a larger x + y + z.
    blocks.sort_by(|a, b| (b.0 + b.1 + b.2).total_cmp(&(a.0 + a.1 + a.2)));

    let mut out = Vec::with_capacity(24);
    for (x, y, z, hero) in blocks {
        let p = project;
        let top = [
            p(x, y, z + 1.0),
            p(x + 1.0, y, z + 1.0),
            p(x + 1.0, y + 1.0, z + 1.0),
            p(x, y + 1.0, z + 1.0),
        ];
        let right = [
            p(x + 1.0, y, z),
            p(x + 1.0, y + 1.0, z),
            p(x + 1.0, y + 1.0, z + 1.0),
            p(x + 1.0, y, z + 1.0),
        ];
        let left = [
            p(x, y + 1.0, z),
            p(x + 1.0, y + 1.0, z),
            p(x + 1.0, y + 1.0, z + 1.0),
            p(x, y + 1.0, z + 1.0),
        ];
        let (t, r, l) = if hero {
            (Tone::Top, Tone::Right, Tone::Left)
        } else {
            (Tone::Kit, Tone::Kit, Tone::Kit)
        };
        out.push((top, t));
        out.push((right, r));
        out.push((left, l));
    }
    out
}

/// Parameter range of `seg` that lies strictly inside the convex `poly`.
fn inside_range(seg: Seg, poly: &[Pt; 4]) -> Option<(f64, f64)> {
    const MARGIN: f64 = 1e-3;
    let ((ax, ay), (bx, by)) = seg;
    let (dx, dy) = (bx - ax, by - ay);
    let area: f64 = (0..4)
        .map(|i| {
            let (p, q) = (poly[i], poly[(i + 1) % 4]);
            p.0 * q.1 - q.0 * p.1
        })
        .sum();
    let sign = area.signum();
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for i in 0..4 {
        let (p, q) = (poly[i], poly[(i + 1) % 4]);
        let (ex, ey) = (q.0 - p.0, q.1 - p.1);
        let len = ex.hypot(ey);
        let (nx, ny) = (-ey * sign / len, ex * sign / len); // inward normal
        let dist = (ax - p.0) * nx + (ay - p.1) * ny - MARGIN;
        let den = dx * nx + dy * ny;
        if den.abs() < 1e-12 {
            if dist <= 0.0 {
                return None;
            }
            continue;
        }
        let t = -dist / den;
        if den > 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 >= t1 {
            return None;
        }
    }
    Some((t0, t1))
}

/// `seg` minus the part of it hidden behind `poly`: zero, one or two pieces.
fn subtract(seg: Seg, poly: &[Pt; 4], out: &mut Vec<Seg>) {
    let Some((t0, t1)) = inside_range(seg, poly) else {
        out.push(seg);
        return;
    };
    let ((ax, ay), (bx, by)) = seg;
    let at = |t: f64| (ax + (bx - ax) * t, ay + (by - ay) * t);
    if t0 > 1e-9 {
        out.push(((ax, ay), at(t0)));
    }
    if t1 < 1.0 - 1e-9 {
        out.push((at(t1), (bx, by)));
    }
}

/// The visible edges of the symbol at `angle`, each with its tone.
pub fn segments(angle: f64) -> Vec<(Seg, Tone)> {
    let faces = faces(lift(angle));
    let mut out = Vec::new();
    for (i, (poly, tone)) in faces.iter().enumerate() {
        let mut pieces: Vec<Seg> = (0..4).map(|k| (poly[k], poly[(k + 1) % 4])).collect();
        for (front, _) in &faces[..i] {
            let mut next = Vec::with_capacity(pieces.len());
            for seg in pieces {
                subtract(seg, front, &mut next);
            }
            pieces = next;
        }
        out.extend(pieces.into_iter().map(|s| (s, *tone)));
    }
    out
}

/// Draw the symbol onto any plotter, in one colour.
pub fn draw<P: Plotter + ?Sized>(p: &mut P, angle: f64) {
    for (seg, _) in segments(angle) {
        p.line(seg.0, seg.1);
    }
}

/// Two-cell pixel cube for the header: amber top over cyan | purple sides.
pub fn header_icon() -> [Span<'static>; 2] {
    [
        Span::styled("▄", Style::new().fg(LEFT).bg(TOP)),
        Span::styled("▄", Style::new().fg(RIGHT).bg(TOP)),
    ]
}

/// Four-by-two-cell hexagonal cube: the top face over the two sides.
pub fn mini() -> [Line<'static>; 2] {
    [
        Line::from(Span::styled("◢██◣", Style::new().fg(TOP))),
        Line::from(vec![
            Span::styled("◥█", Style::new().fg(LEFT)),
            Span::styled("█◤", Style::new().fg(RIGHT)),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_its_bounds_at_every_angle() {
        let (min_x, max_x) = (CENTER.0 - RADIUS, CENTER.0 + RADIUS);
        let (min_y, max_y) = (CENTER.1 - RADIUS, CENTER.1 + RADIUS);
        for step in 0..64 {
            let angle = f64::from(step) * std::f64::consts::TAU / 64.0;
            for ((a, b), _) in segments(angle) {
                for (x, y) in [a, b] {
                    assert!((min_x..=max_x).contains(&x), "x {x} out of bounds");
                    assert!((min_y..=max_y).contains(&y), "y {y} out of bounds");
                }
            }
        }
    }

    #[test]
    fn the_lifted_block_shows_all_three_faces() {
        let segs = segments(0.0);
        for tone in [Tone::Top, Tone::Left, Tone::Right] {
            assert!(segs.iter().any(|(_, t)| *t == tone), "{tone:?} missing");
        }
        assert!(segs.iter().any(|(_, t)| *t == Tone::Kit));
    }

    #[test]
    fn hidden_edges_are_removed() {
        // Unclipped, 8 blocks × 3 faces × 4 edges = 96 segments. Hidden-line
        // removal must drop or split a good share of them.
        let segs = segments(0.0);
        let total: f64 = segs
            .iter()
            .map(|((a, b), _)| (b.0 - a.0).hypot(b.1 - a.1))
            .sum();
        let unclipped: f64 = faces(lift(0.0))
            .iter()
            .flat_map(|(p, _)| (0..4).map(move |k| (p[k], p[(k + 1) % 4])))
            .map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1))
            .sum();
        assert!(total < unclipped * 0.8, "{total} vs {unclipped}");
    }

    #[test]
    fn the_block_hovers_but_never_lands() {
        for step in 0..64 {
            let l = lift(f64::from(step) * 0.1);
            assert!(l > 0.4 && l < 0.8, "lift {l}");
        }
    }

    #[test]
    fn cell_icons_have_fixed_widths() {
        assert_eq!(header_icon().iter().map(|s| s.width()).sum::<usize>(), 2);
        for line in mini() {
            assert_eq!(line.width(), 4);
        }
    }
}
