//! Braille preview drawings for the detail pane.
//!
//! Drawings are expressed against [`Plotter`] in dot units (y up), recorded,
//! and then scaled by [`fit_bounds`] to fill whatever space the pane has.

use makit_geometry::drawing::{
    draw_arrow, draw_isometric_box, draw_rect, fill_rect, isometric_z_up,
};
use makit_geometry::{Plotter, Recording, Rectangle};
use makit_tools::building_model::{BuildingModel, GenericWall, Vec3, WwrAnalysis};
use makit_tools::murb::MurbResults;

use crate::logo;
use crate::node::PreviewKind;

/// Month initials for the energy chart.
const MONTHS: [&str; 12] = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];

/// Wall rose geometry (dot units): centre, length of the longest spoke,
/// and how far past the rim the compass letters sit.
const ROSE_CENTER: (f64, f64) = (20.0, 16.0);
const ROSE_RADIUS: f64 = 12.0;
const ROSE_LABEL_GAP: f64 = 3.0;
/// Half-width of a rose spoke.
const ROSE_SPOKE_HALF_WIDTH: f64 = 0.7;
/// Compass points of the rose: analysis name, letter, unit direction.
const COMPASS: [(&str, &str, (f64, f64)); 4] = [
    ("North", "N", (0.0, 1.0)),
    ("East", "E", (1.0, 0.0)),
    ("South", "S", (0.0, -1.0)),
    ("West", "W", (-1.0, 0.0)),
];

/// Real data a preview draws in place of an illustration.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreviewData<'a> {
    /// Results of the last MURB simulation.
    pub murb: Option<&'a MurbResults>,
    /// Wall analysis of the loaded IFC file, when the focused node is about
    /// that file.
    pub wwr: Option<&'a WwrAnalysis>,
    /// The loaded IFC model, when the focused node is about that file.
    pub model: Option<&'a BuildingModel>,
}

impl<'a> PreviewData<'a> {
    /// The loaded model when it can be drawn: a format 3 extraction with
    /// wall axes.
    pub fn building(&self) -> Option<&'a BuildingModel> {
        self.model.filter(|m| m.has_axes())
    }
}

/// Logo geometry (centre and radius in dot units).
const LOGO_CENTER: (f64, f64) = logo::CENTER;
const LOGO_RADIUS: f64 = logo::RADIUS;

/// Largest sub-dot expansion per axis when scaling filled shapes up.
const MAX_FILL_EXPANSION: usize = 8;

/// A preview drawing in two tones: `main` in the accent colour, drawn over
/// `faint` in the dim colour (a building's interior walls under its
/// exterior ones).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub main: Recording,
    pub faint: Recording,
}

impl Scene {
    /// True when neither layer has anything in it.
    pub fn is_empty(&self) -> bool {
        self.main.is_empty() && self.faint.is_empty()
    }

    /// Bounds of both layers together.
    pub fn bounds(&self) -> Option<Rectangle> {
        match (self.main.bounds(), self.faint.bounds()) {
            (Some(a), Some(b)) => Some(a.union(&b)),
            (a, b) => a.or(b),
        }
    }
}

/// Record the preview for `kind`: the loaded building for a drawable
/// model, otherwise whatever [`draw_preview`] draws, in the main layer.
pub fn record(kind: PreviewKind, angle: f64, data: PreviewData) -> Scene {
    let mut scene = Scene::default();
    match (kind, data.building()) {
        (PreviewKind::Building, Some(model)) => {
            draw_building(&mut scene.main, &mut scene.faint, model)
        }
        _ => draw_preview(kind, &mut scene.main, angle, data),
    }
    scene
}

/// A wall the isometric can draw: the ends of its axis, its bottom and its
/// top. The drawing and the caption agree through this one function.
fn panel(wall: &GenericWall) -> Option<(Vec3, Vec3, f64, f64)> {
    let (start, end) = wall.axis()?;
    Some((start, end, wall.bottom?, wall.top?))
}

/// The loaded model's walls as standing panels in a 30° isometric: each
/// axis lifted from the wall's bottom to its top, four edges per wall.
/// Exterior walls go into `main`, interior ones (`isExternal: false`) into
/// `faint`, so the envelope reads over the partitions.
fn draw_building<P: Plotter + ?Sized>(main: &mut P, faint: &mut P, model: &BuildingModel) {
    for wall in &model.walls {
        let Some((start, end, bottom, top)) = panel(wall) else {
            continue;
        };
        let corners = [
            isometric_z_up(start.x, start.y, bottom),
            isometric_z_up(end.x, end.y, bottom),
            isometric_z_up(end.x, end.y, top),
            isometric_z_up(start.x, start.y, top),
        ];
        let layer: &mut P = if wall.is_external == Some(false) {
            &mut *faint
        } else {
            &mut *main
        };
        for k in 0..4 {
            layer.line(corners[k], corners[(k + 1) % 4]);
        }
    }
}

/// What the building drawing covers, for its caption.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BuildingSummary {
    /// Walls drawn: with an axis and both elevations.
    pub drawn: usize,
    /// Walls left out for want of an axis or an elevation.
    pub skipped: usize,
    /// Extent of the drawn axes along x, metres.
    pub width: f64,
    /// Extent of the drawn axes along y, metres.
    pub depth: f64,
    /// Highest top above the lowest bottom, metres.
    pub height: f64,
}

/// Count and measure the walls [`draw_building`] draws.
pub fn building_summary(model: &BuildingModel) -> BuildingSummary {
    let mut summary = BuildingSummary::default();
    let mut x = (f64::INFINITY, f64::NEG_INFINITY);
    let mut y = x;
    let mut z = x;
    for wall in &model.walls {
        match panel(wall) {
            Some((start, end, bottom, top)) => {
                summary.drawn += 1;
                for p in [start, end] {
                    x = (x.0.min(p.x), x.1.max(p.x));
                    y = (y.0.min(p.y), y.1.max(p.y));
                }
                z = (z.0.min(bottom), z.1.max(top));
            }
            None => summary.skipped += 1,
        }
    }
    if summary.drawn > 0 {
        summary.width = x.1 - x.0;
        summary.depth = y.1 - y.0;
        summary.height = z.1 - z.0;
    }
    summary
}

/// Draw the preview for `kind` onto any plotter: from `data` where there
/// is some, otherwise a fixed illustration (or an empty chart). The
/// building drawing needs two layers and lives in [`record`].
pub fn draw_preview<P: Plotter + ?Sized>(
    kind: PreviewKind,
    p: &mut P,
    angle: f64,
    data: PreviewData,
) {
    match (kind, data.wwr) {
        (PreviewKind::Energy, _) => draw_energy_bars(p, data.murb),
        (PreviewKind::Walls, Some(analysis)) => draw_wall_rose(p, analysis),
        (PreviewKind::Walls, None) => draw_walls(p),
        (PreviewKind::Building, _) => draw_isometric_building(p),
        (PreviewKind::FloorPlan, _) => draw_floor_plan(p),
        (PreviewKind::Logo, _) => draw_logo(p, angle),
    }
}

/// Heading for the preview. Drawings that do not come from the user's data
/// say so, so a stock picture is never taken for a result.
pub fn title(kind: PreviewKind, data: PreviewData) -> &'static str {
    match kind {
        PreviewKind::Energy if data.murb.is_some() => "heating demand",
        PreviewKind::Energy => "heating demand · no results",
        PreviewKind::Walls if data.wwr.is_some() => "wall area by orientation",
        PreviewKind::Walls => "wall orientation · illustration",
        PreviewKind::Building if data.building().is_some() => "isometric",
        PreviewKind::Building => "isometric · illustration",
        PreviewKind::FloorPlan => "floor plan · illustration",
        PreviewKind::Logo => "makit",
    }
}

/// Text lines shown under the preview.
pub fn caption(kind: PreviewKind, data: PreviewData) -> Vec<String> {
    match (kind, data.murb, data.building()) {
        (PreviewKind::Energy, Some(r), _) => vec![
            format!("TEDI  {:.1} kWh/m²", r.tedi_kwh_m2),
            format!("TEUI  {:.1} kWh/m²", r.teui_kwh_m2),
            format!("GHGI  {:.1} kgCO₂/m²", r.ghgi_kg_m2),
        ],
        (PreviewKind::Energy, None, _) => vec!["No results — run murb-simulate".to_string()],
        (PreviewKind::Building, _, Some(model)) => building_caption(&building_summary(model)),
        _ => Vec::new(),
    }
}

/// The two caption lines under a drawn building: what was drawn (and what
/// could not be), and how big it is.
fn building_caption(s: &BuildingSummary) -> Vec<String> {
    let walls = if s.skipped > 0 {
        format!(
            "Walls {} drawn · {} without axis or height",
            s.drawn, s.skipped
        )
    } else {
        format!("Walls {} drawn", s.drawn)
    };
    vec![
        walls,
        format!(
            "{:.1} × {:.1} m footprint · {:.1} m high",
            s.width, s.depth, s.height
        ),
    ]
}

/// Bounds to fit for a scene. The logo uses fixed bounds so it does not
/// "breathe" as its block hovers.
pub fn content_bounds(kind: PreviewKind, scene: &Scene) -> Option<Rectangle> {
    match kind {
        PreviewKind::Logo => Some(Rectangle::new(
            LOGO_CENTER.0 - LOGO_RADIUS,
            LOGO_CENTER.1 - LOGO_RADIUS,
            2.0 * LOGO_RADIUS,
            2.0 * LOGO_RADIUS,
        )),
        _ => scene.bounds(),
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

/// The makit symbol: a kit of blocks with the front-top block lifted out,
/// hovering with `angle`. See [`logo`].
fn draw_logo<P: Plotter + ?Sized>(c: &mut P, angle: f64) {
    logo::draw(c, angle);
}

/// Isometric wireframe building — 10×8 m footprint with a floor division and parapet.
fn draw_isometric_building<P: Plotter + ?Sized>(c: &mut P) {
    let (ox, oy) = (20.0, 28.0);
    draw_isometric_box(c, ox, oy, 10.0, 6.0, 8.0);
    draw_isometric_box(c, ox, oy, 10.0, 3.0, 8.0);
    draw_isometric_box(c, ox, oy, 10.0, 7.0, 8.0);
}

/// Length of the rose's North, East, South and West spokes: each
/// direction's share of the wall area, with the largest reaching the rim.
fn spoke_lengths(analysis: &WwrAnalysis) -> [f64; 4] {
    let shares = COMPASS.map(|(name, _, _)| {
        analysis
            .orientations
            .iter()
            .find(|o| o.direction == name)
            .map_or(0.0, |o| o.percentage.max(0.0))
    });
    let largest = shares.iter().copied().fold(0.0_f64, f64::max);
    if largest <= 0.0 {
        return [0.0; 4];
    }
    shares.map(|share| share / largest * ROSE_RADIUS)
}

/// Wall area by facing direction: thin compass axes with a thick spoke per
/// direction, as long as that direction's share of the wall area.
fn draw_wall_rose<P: Plotter + ?Sized>(c: &mut P, analysis: &WwrAnalysis) {
    let (cx, cy) = ROSE_CENTER;
    let lengths = spoke_lengths(analysis);
    for ((_, letter, (dx, dy)), length) in COMPASS.into_iter().zip(lengths) {
        c.line((cx, cy), (cx + dx * ROSE_RADIUS, cy + dy * ROSE_RADIUS));
        if length > 0.0 {
            // Perpendicular offsets turn the spoke into a bar with a cap.
            let (px, py) = (-dy * ROSE_SPOKE_HALF_WIDTH, dx * ROSE_SPOKE_HALF_WIDTH);
            let tip = (cx + dx * length, cy + dy * length);
            c.line((cx + px, cy + py), (tip.0 + px, tip.1 + py));
            c.line((cx - px, cy - py), (tip.0 - px, tip.1 - py));
            c.line((tip.0 + px, tip.1 + py), (tip.0 - px, tip.1 - py));
        }
        let rim = ROSE_RADIUS + ROSE_LABEL_GAP;
        c.label(cx + dx * rim, cy + dy * rim, letter);
    }
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

/// Monthly heating demand bars from MURB results. Without results only
/// the empty axes are drawn — never made-up bars.
fn draw_energy_bars<P: Plotter + ?Sized>(c: &mut P, results: Option<&MurbResults>) {
    let chart_height = 28.0;
    if let Some(r) = results {
        let max_val = r
            .monthly_heating_demand
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
            .max(1.0);
        for (i, v) in r.monthly_heating_demand.iter().enumerate().take(12) {
            let h = ((v / max_val) * chart_height) as i32;
            if h > 0 {
                fill_rect(c, i as i32 * 3 + 2, 0, 2, h);
            }
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
    use makit_tools::revit::models::OrientationResult;

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
            breakdown: None,
            input: None,
            envelope: None,
            model: None,
            even_glass: None,
            command: None,
        }
    }

    fn with_murb(results: &MurbResults) -> PreviewData<'_> {
        PreviewData {
            murb: Some(results),
            ..PreviewData::default()
        }
    }

    /// An analysis with the given share of wall area per direction.
    fn wwr(shares: &[(&str, f64)]) -> WwrAnalysis {
        WwrAnalysis {
            orientations: shares
                .iter()
                .map(|(direction, percentage)| OrientationResult {
                    direction: direction.to_string(),
                    count: 1,
                    total_area_sqm: *percentage,
                    percentage: *percentage,
                })
                .collect(),
            total_wall_area: 100.0,
            total_window_area: 20.0,
            overall_wwr: 20.0,
            wall_count: shares.len(),
            window_count: 2,
            project_name: String::new(),
            interior_walls_left_out: 0,
        }
    }

    fn with_wwr(analysis: &WwrAnalysis) -> PreviewData<'_> {
        PreviewData {
            wwr: Some(analysis),
            ..PreviewData::default()
        }
    }

    const ALL: [PreviewKind; 5] = [
        PreviewKind::Energy,
        PreviewKind::Walls,
        PreviewKind::Building,
        PreviewKind::FloorPlan,
        PreviewKind::Logo,
    ];

    /// A test wall: start xy, end xy, bottom, top, exterior.
    type TestWall = ((f64, f64), (f64, f64), Option<f64>, Option<f64>, bool);

    /// A format 3 model of `walls`; `None` for a wall with no axis.
    fn model(walls: &[Option<TestWall>]) -> BuildingModel {
        let walls: Vec<serde_json::Value> = walls
            .iter()
            .map(|w| match w {
                Some(((x0, y0), (x1, y1), bottom, top, exterior)) => serde_json::json!({
                    "startPoint": {"x": x0, "y": y0, "z": 0.0}, "endPoint": {"x": x1, "y": y1, "z": 0.0},
                    "bottom": bottom, "top": top, "isExternal": exterior
                }),
                None => serde_json::json!({}),
            })
            .collect();
        serde_json::from_value(
            serde_json::json!({ "walls": walls, "metadata": { "formatVersion": 3 } }),
        )
        .unwrap()
    }

    fn with_model(model: &BuildingModel) -> PreviewData<'_> {
        PreviewData {
            model: Some(model),
            ..PreviewData::default()
        }
    }

    #[test]
    fn a_model_with_axes_is_drawn_as_standing_panels() {
        let m = model(&[
            Some(((0.0, 0.0), (10.0, 0.0), Some(0.0), Some(3.0), true)),
            Some(((5.0, 0.0), (5.0, 8.0), Some(0.0), Some(3.0), false)),
        ]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert_eq!(
            (scene.main.segments.len(), scene.faint.segments.len()),
            (4, 4)
        );
        // The exterior wall's bottom edge runs east: right and up on screen.
        let (a, b) = scene.main.segments[0];
        assert_eq!(a, (0.0, 0.0));
        assert!((b.0 - 8.660_254_038).abs() < 1e-9 && (b.1 - 5.0).abs() < 1e-9);
        assert!(scene.main.points.is_empty());
        let s = building_summary(&m);
        assert_eq!((s.drawn, s.skipped), (2, 0));
        assert_eq!((s.width, s.depth, s.height), (10.0, 8.0, 3.0));
        assert_eq!(title(PreviewKind::Building, with_model(&m)), "isometric");
        assert_eq!(
            caption(PreviewKind::Building, with_model(&m)),
            vec!["Walls 2 drawn", "10.0 × 8.0 m footprint · 3.0 m high"]
        );
    }

    #[test]
    fn walls_without_an_axis_or_height_are_counted_not_drawn() {
        let m = model(&[
            Some(((0.0, 0.0), (4.0, 0.0), Some(0.0), Some(2.5), true)),
            Some(((0.0, 0.0), (0.0, 4.0), None, Some(2.5), true)),
            None,
        ]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert_eq!(scene.main.segments.len(), 4);
        assert_eq!(building_summary(&m).skipped, 2);
        assert_eq!(
            caption(PreviewKind::Building, with_model(&m))[0],
            "Walls 1 drawn · 2 without axis or height"
        );
    }

    #[test]
    fn an_all_interior_model_still_fills_the_pane() {
        let m = model(&[Some(((0.0, 0.0), (6.0, 0.0), Some(0.0), Some(3.0), false))]);
        let scene = record(PreviewKind::Building, 0.0, with_model(&m));
        assert!(scene.main.is_empty() && !scene.faint.is_empty());
        let bounds = content_bounds(PreviewKind::Building, &scene).unwrap();
        assert!(bounds.width > 0.0 && bounds.height > 0.0);
    }

    #[test]
    fn a_model_without_axes_is_the_illustration() {
        let two: BuildingModel = serde_json::from_str(
            r#"{ "walls": [{ "startPoint": {"x": 0.0, "y": 0.0, "z": 0.0}, "endPoint": {"x": 4.0, "y": 0.0, "z": 0.0} }],
                 "metadata": { "formatVersion": 2 } }"#,
        )
        .unwrap();
        let real = record(PreviewKind::Building, 0.0, with_model(&two));
        assert_eq!(
            real,
            record(PreviewKind::Building, 0.0, PreviewData::default())
        );
        assert_eq!(
            title(PreviewKind::Building, with_model(&two)),
            "isometric · illustration"
        );
        assert!(caption(PreviewKind::Building, with_model(&two)).is_empty());
    }

    #[test]
    fn the_sample_house_draws_every_wall() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/Ifc4_ARC.json"
        );
        let m = makit_tools::building_model::load_model(std::path::Path::new(path)).unwrap();
        let s = building_summary(&m);
        assert_eq!(s.drawn + s.skipped, m.walls.len());
        assert!(s.drawn >= 47, "{s:?}");
        assert!(s.height > 9.0 && s.height < 12.0, "{s:?}");
    }

    #[test]
    fn every_preview_draws_something() {
        for kind in ALL {
            let rec = record(kind, 0.3, PreviewData::default());
            assert!(!rec.is_empty(), "{kind:?} drew nothing");
            assert!(content_bounds(kind, &rec).is_some());
        }
    }

    #[test]
    fn logo_moves_but_its_bounds_do_not() {
        let a = record(PreviewKind::Logo, 0.0, PreviewData::default());
        let b = record(PreviewKind::Logo, 0.5, PreviewData::default());
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
        let results = murb(heating);
        let rec = record(PreviewKind::Energy, 0.0, with_murb(&results));
        let max_y_at = |x: f64| {
            rec.main
                .points
                .iter()
                .filter(|p| p.0 == x)
                .map(|p| p.1)
                .fold(f64::MIN, f64::max)
        };
        assert_eq!(max_y_at(2.0), 28.0); // January: full height
        assert_eq!(max_y_at(20.0), 14.0); // July: half height
        assert_eq!(rec.main.labels.len(), 12);
    }

    #[test]
    fn captions_reflect_results() {
        let results = murb(vec![1.0; 12]);
        let with = caption(PreviewKind::Energy, with_murb(&results));
        assert_eq!(with[0], "TEDI  12.3 kWh/m²");
        assert_eq!(with.len(), 3);
        assert_eq!(
            caption(PreviewKind::Energy, PreviewData::default()),
            vec!["No results — run murb-simulate"]
        );
        assert!(caption(PreviewKind::Walls, PreviewData::default()).is_empty());
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

    // ─── Honest previews ────────────────────────────────────────────────

    #[test]
    fn energy_without_results_draws_the_axes_and_no_bars() {
        let rec = record(PreviewKind::Energy, 0.0, PreviewData::default());
        // Bars are filled dots; an empty chart has none to mistake for data.
        assert!(
            rec.main.points.is_empty(),
            "{} dots drawn",
            rec.main.points.len()
        );
        assert_eq!(rec.main.segments.len(), 2);
        assert_eq!(rec.main.labels.len(), 12);
    }

    #[test]
    fn wall_rose_spokes_follow_each_directions_share() {
        let analysis = wwr(&[("North", 50.0), ("East", 25.0), ("South", 25.0)]);
        // North, East, South, West — the largest share reaches the rim.
        assert_eq!(
            spoke_lengths(&analysis),
            [ROSE_RADIUS, ROSE_RADIUS / 2.0, ROSE_RADIUS / 2.0, 0.0]
        );
        // Directions the compass has no spoke for are left to the table.
        let odd = wwr(&[("Unknown", 80.0), ("West", 20.0)]);
        assert_eq!(spoke_lengths(&odd), [0.0, 0.0, 0.0, ROSE_RADIUS]);
        // No walls: nothing to draw, and nothing divides by zero.
        assert_eq!(spoke_lengths(&wwr(&[])), [0.0; 4]);
    }

    #[test]
    fn walls_preview_draws_the_analysis_when_there_is_one() {
        let analysis = wwr(&[("North", 60.0), ("West", 40.0)]);
        let real = record(PreviewKind::Walls, 0.0, with_wwr(&analysis));
        let illustration = record(PreviewKind::Walls, 0.0, PreviewData::default());
        assert_ne!(real, illustration);
        let labels: Vec<&str> = real.main.labels.iter().map(|l| l.2.as_str()).collect();
        assert_eq!(labels, vec!["N", "E", "S", "W"]);
        // Different data, different drawing.
        let other = wwr(&[("North", 10.0), ("West", 90.0)]);
        assert_ne!(real, record(PreviewKind::Walls, 0.0, with_wwr(&other)));
        // The rose stays inside its rim (plus the letters around it).
        let bounds = real.bounds().unwrap();
        assert!(bounds.width <= 2.0 * (ROSE_RADIUS + ROSE_LABEL_GAP) + 1e-9);
    }

    #[test]
    fn titles_say_which_previews_are_real() {
        let results = murb(vec![1.0; 12]);
        let analysis = wwr(&[("North", 100.0)]);
        let none = PreviewData::default();
        assert_eq!(
            title(PreviewKind::Energy, with_murb(&results)),
            "heating demand"
        );
        assert_eq!(
            title(PreviewKind::Energy, none),
            "heating demand · no results"
        );
        assert_eq!(
            title(PreviewKind::Walls, with_wwr(&analysis)),
            "wall area by orientation"
        );
        assert_eq!(
            title(PreviewKind::Walls, none),
            "wall orientation · illustration"
        );
        assert_eq!(
            title(PreviewKind::Building, none),
            "isometric · illustration"
        );
        assert_eq!(
            title(PreviewKind::FloorPlan, none),
            "floor plan · illustration"
        );
        assert_eq!(title(PreviewKind::Logo, none), "makit");
    }
}
