//! Braille preview drawings for the detail pane.
//!
//! Drawings are expressed against [`Plotter`] in dot units (y up), recorded,
//! and then scaled by [`fit_bounds`] to fill whatever space the pane has.

use makit_geometry::drawing::{draw_arrow, draw_isometric_box, draw_rect, fill_rect};
use makit_geometry::{Plotter, Recording, Rectangle};
use makit_tools::murb::MurbResults;

use crate::node::PreviewKind;

/// Month initials for the energy chart.
const MONTHS: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];

/// Placeholder heating profile shown before any MURB results exist.
const PLACEHOLDER_BARS: [i32; 12] = [28, 24, 18, 10, 5, 2, 1, 2, 6, 14, 22, 26];

/// Logo geometry (centre and radius in dot units).
const LOGO_CENTER: (f64, f64) = (20.0, 16.0);
const LOGO_RADIUS: f64 = 12.0;

/// Largest sub-dot expansion per axis when scaling filled shapes up.
const MAX_FILL_EXPANSION: usize = 8;

/// Record the preview for `kind`.
pub fn record(kind: PreviewKind, angle: f64, murb: Option<&MurbResults>) -> Recording {
    let mut rec = Recording::new();
    draw_preview(kind, &mut rec, angle, murb);
    rec
}

/// Draw the preview for `kind` onto any plotter.
pub fn draw_preview<P: Plotter + ?Sized>(
    kind: PreviewKind,
    p: &mut P,
    angle: f64,
    murb: Option<&MurbResults>,
) {
    match kind {
        PreviewKind::Energy => draw_energy_bars(p, murb),
        PreviewKind::Walls => draw_walls(p),
        PreviewKind::Building => draw_isometric_building(p),
        PreviewKind::FloorPlan => draw_floor_plan(p),
        PreviewKind::Logo => draw_logo(p, angle),
    }
}

/// Text lines shown under the preview.
pub fn caption(kind: PreviewKind, murb: Option<&MurbResults>) -> Vec<String> {
    match (kind, murb) {
        (PreviewKind::Energy, Some(r)) => vec![
            format!("TEDI  {:.1} kWh/m²", r.tedi_kwh_m2),
            format!("TEUI  {:.1} kWh/m²", r.teui_kwh_m2),
            format!("GHGI  {:.1} kgCO₂/m²", r.ghgi_kg_m2),
        ],
        (PreviewKind::Energy, None) => vec!["No results — run murb-simulate".to_string()],
        _ => Vec::new(),
    }
}

/// Bounds to fit for a recording. The logo uses fixed bounds so it does not
/// "breathe" as it rotates.
pub fn content_bounds(kind: PreviewKind, rec: &Recording) -> Option<Rectangle> {
    match kind {
        PreviewKind::Logo => Some(Rectangle::new(
            LOGO_CENTER.0 - LOGO_RADIUS,
            LOGO_CENTER.1 - LOGO_RADIUS,
            2.0 * LOGO_RADIUS,
            2.0 * LOGO_RADIUS,
        )),
        _ => rec.bounds(),
    }
}

/// Canvas bounds plus the resulting scale (braille dots per data unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub x_bounds: [f64; 2],
    pub y_bounds: [f64; 2],
    pub scale: f64,
}

/// Choose canvas bounds that show `content` (padded by one unit) as large as
/// possible in a `cols`×`rows` braille area, preserving aspect ratio in dot
/// space and centring the drawing.
pub fn fit_bounds(content: Rectangle, cols: u16, rows: u16) -> Fit {
    let pad = 1.0;
    let w = content.width.max(1.0) + 2.0 * pad;
    let h = content.height.max(1.0) + 2.0 * pad;
    let cx = content.x + content.width / 2.0;
    let cy = content.y + content.height / 2.0;

    let dots_x = f64::from(cols) * 2.0;
    let dots_y = f64::from(rows) * 4.0;
    if dots_x <= 0.0 || dots_y <= 0.0 {
        return Fit {
            x_bounds: [cx - w / 2.0, cx + w / 2.0],
            y_bounds: [cy - h / 2.0, cy + h / 2.0],
            scale: 1.0,
        };
    }

    let scale = (dots_x / w).min(dots_y / h);
    let span_x = dots_x / scale;
    let span_y = dots_y / scale;
    Fit {
        x_bounds: [cx - span_x / 2.0, cx + span_x / 2.0],
        y_bounds: [cy - span_y / 2.0, cy + span_y / 2.0],
        scale,
    }
}

/// Expand single dots into `k×k` sub-dots (k = ⌈scale⌉) so filled shapes stay
/// solid when drawn larger than 1:1.
pub fn expand_points(points: &[(f64, f64)], scale: f64) -> Vec<(f64, f64)> {
    let k = (scale.ceil() as usize).clamp(1, MAX_FILL_EXPANSION);
    if k == 1 {
        return points.to_vec();
    }
    let step = 1.0 / scale;
    let offsets: Vec<f64> = (0..k).map(|i| -0.5 + (i as f64 + 0.5) * step).collect();
    let mut out = Vec::with_capacity(points.len() * k * k);
    for &(x, y) in points {
        for &dx in &offsets {
            for &dy in &offsets {
                out.push((x + dx, y + dy));
            }
        }
    }
    out
}

/// Rotating hexagonal logo with inner counter-rotation and spokes.
fn draw_logo<P: Plotter + ?Sized>(c: &mut P, angle: f64) {
    let (cx, cy) = LOGO_CENTER;
    let r = LOGO_RADIUS;
    let r2 = r * 0.55;
    let sides = 6;
    let vertex = |radius: f64, a: f64| (cx + radius * a.cos(), cy + radius * a.sin());
    let step = std::f64::consts::TAU / sides as f64;

    for i in 0..sides {
        let a1 = angle + i as f64 * step;
        let a2 = angle + (i + 1) as f64 * step;
        c.line(vertex(r, a1), vertex(r, a2));

        let b1 = -angle + i as f64 * step;
        let b2 = -angle + (i + 1) as f64 * step;
        c.line(vertex(r2, b1), vertex(r2, b2));

        c.line(vertex(r2, -a1), vertex(r, a1));
    }
}

/// Isometric wireframe building — 10×8 m footprint with a floor division and parapet.
fn draw_isometric_building<P: Plotter + ?Sized>(c: &mut P) {
    let (ox, oy) = (20.0, 28.0);
    draw_isometric_box(c, ox, oy, 10.0, 6.0, 8.0);
    draw_isometric_box(c, ox, oy, 10.0, 3.0, 8.0);
    draw_isometric_box(c, ox, oy, 10.0, 7.0, 8.0);
}

/// Building envelope with interior walls and cardinal arrows.
fn draw_walls<P: Plotter + ?Sized>(c: &mut P) {
    draw_rect(c, 5.0, 4.0, 30.0, 24.0);
    c.line((20.0, 4.0), (20.0, 20.0));
    c.line((5.0, 16.0), (20.0, 16.0));
    draw_arrow(c, 20.0, 32.0, 20.0, 36.0);
    draw_arrow(c, 40.0, 16.0, 44.0, 16.0);
    c.label(20.0, 38.0, "N");
    c.label(46.0, 16.0, "E");
}

/// Monthly heating demand bars — real MURB results when available.
fn draw_energy_bars<P: Plotter + ?Sized>(c: &mut P, results: Option<&MurbResults>) {
    let chart_height = 28.0;
    let heights: Vec<i32> = match results {
        Some(r) => {
            let max_val = r
                .monthly_heating_demand
                .iter()
                .copied()
                .fold(0.0_f64, f64::max)
                .max(1.0);
            r.monthly_heating_demand
                .iter()
                .map(|v| ((v / max_val) * chart_height) as i32)
                .collect()
        }
        None => PLACEHOLDER_BARS.to_vec(),
    };

    for (i, &h) in heights.iter().enumerate().take(12) {
        let x = i as i32 * 3 + 2;
        if h > 0 {
            fill_rect(c, x, 0, 2, h);
        }
    }
    c.line((0.0, 0.0), (40.0, 0.0));
    c.line((0.0, 0.0), (0.0, 32.0));
    for (i, month) in MONTHS.iter().enumerate() {
        c.label(i as f64 * 3.0 + 3.0, -4.0, month);
    }
}

/// Floor plan — rooms with door openings.
fn draw_floor_plan<P: Plotter + ?Sized>(c: &mut P) {
    draw_rect(c, 2.0, 2.0, 36.0, 28.0);
    c.line((20.0, 2.0), (20.0, 22.0));
    c.line((2.0, 18.0), (36.0, 18.0));
    c.line((20.0, 24.0), (20.0, 30.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn murb(heating: Vec<f64>) -> MurbResults {
        MurbResults {
            name: "test".into(),
            monthly_heating_demand: heating,
            monthly_cooling_demand: vec![],
            monthly_heating_consumption: vec![],
            monthly_cooling_consumption: vec![],
            monthly_total_consumption: vec![],
            monthly_ghg_emissions: vec![],
            tedi_kwh_m2: 12.34,
            teui_kwh_m2: 56.7,
            ghgi_kg_m2: 3.21,
            gfa: 1000.0,
            weather_file: String::new(),
        }
    }

    const ALL: [PreviewKind; 5] = [
        PreviewKind::Energy,
        PreviewKind::Walls,
        PreviewKind::Building,
        PreviewKind::FloorPlan,
        PreviewKind::Logo,
    ];

    #[test]
    fn every_preview_draws_something() {
        for kind in ALL {
            let rec = record(kind, 0.3, None);
            assert!(!rec.is_empty(), "{kind:?} drew nothing");
            assert!(content_bounds(kind, &rec).is_some());
        }
    }

    #[test]
    fn logo_moves_but_its_bounds_do_not() {
        let a = record(PreviewKind::Logo, 0.0, None);
        let b = record(PreviewKind::Logo, 0.5, None);
        assert_ne!(a, b);
        assert_eq!(
            content_bounds(PreviewKind::Logo, &a),
            content_bounds(PreviewKind::Logo, &b)
        );
    }

    #[test]
    fn energy_bars_scale_to_the_largest_month() {
        let mut heating = vec![0.0; 12];
        heating[0] = 100.0;
        heating[6] = 50.0;
        let rec = record(PreviewKind::Energy, 0.0, Some(&murb(heating)));
        let max_y_at = |x: f64| {
            rec.points
                .iter()
                .filter(|p| p.0 == x)
                .map(|p| p.1)
                .fold(f64::MIN, f64::max)
        };
        assert_eq!(max_y_at(2.0), 28.0); // January: full height
        assert_eq!(max_y_at(20.0), 14.0); // July: half height
        assert_eq!(rec.labels.len(), 12);
    }

    #[test]
    fn captions_reflect_results() {
        let with = caption(PreviewKind::Energy, Some(&murb(vec![1.0; 12])));
        assert_eq!(with[0], "TEDI  12.3 kWh/m²");
        assert_eq!(with.len(), 3);
        assert_eq!(
            caption(PreviewKind::Energy, None),
            vec!["No results — run murb-simulate"]
        );
        assert!(caption(PreviewKind::Walls, None).is_empty());
    }

    #[test]
    fn fit_fills_the_tighter_axis_and_keeps_aspect() {
        let content = Rectangle::new(0.0, 0.0, 40.0, 32.0);
        let fit = fit_bounds(content, 60, 24); // 120×96 dots
        let span_x = fit.x_bounds[1] - fit.x_bounds[0];
        let span_y = fit.y_bounds[1] - fit.y_bounds[0];
        assert!((span_x * fit.scale - 120.0).abs() < 1e-9);
        assert!((span_y * fit.scale - 96.0).abs() < 1e-9);
        // Height (32 + 2 padding) is the limiting axis.
        assert!((fit.scale - 96.0 / 34.0).abs() < 1e-9);
        // Content is centred and fully visible.
        assert!(fit.x_bounds[0] <= 0.0 && fit.x_bounds[1] >= 40.0);
        assert!(fit.y_bounds[0] <= 0.0 && fit.y_bounds[1] >= 32.0);
        let mid_x = (fit.x_bounds[0] + fit.x_bounds[1]) / 2.0;
        assert!((mid_x - 20.0).abs() < 1e-9);
    }

    #[test]
    fn fit_handles_degenerate_inputs() {
        let point = Rectangle::new(5.0, 5.0, 0.0, 0.0);
        let fit = fit_bounds(point, 10, 5);
        assert!(fit.scale.is_finite() && fit.scale > 0.0);
        let zero = fit_bounds(Rectangle::new(0.0, 0.0, 10.0, 10.0), 0, 0);
        assert_eq!(zero.scale, 1.0);
        assert!(zero.x_bounds[0] < zero.x_bounds[1]);
    }

    #[test]
    fn expansion_keeps_fills_solid() {
        let pts = [(0.0, 0.0)];
        assert_eq!(expand_points(&pts, 1.0), pts.to_vec());
        assert_eq!(expand_points(&pts, 0.5), pts.to_vec());
        let big = expand_points(&pts, 2.5);
        assert_eq!(big.len(), 9);
        assert!(big.iter().all(|p| p.0.abs() <= 0.5 && p.1.abs() <= 0.5));
        // Capped so huge panes don't explode the point count.
        assert_eq!(
            expand_points(&pts, 100.0).len(),
            MAX_FILL_EXPANSION * MAX_FILL_EXPANSION
        );
    }
}
