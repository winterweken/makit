//! `view(frame, state)` — render the whole UI from state. No mutation.

use std::path::Path;

use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Row, Table,
};
use ratatui::Frame;

use crate::app::{AppState, Focus};
use crate::layout::{self, AppLayout, IfcLayout};
use crate::node::PreviewKind;
use crate::preview;
use crate::theme::THEME;

/// Braille spinner frames for running work.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Help overlay size.
const HELP_WIDTH: u16 = 62;
const HELP_HEIGHT: u16 = 25;

/// Render the full UI.
pub fn view(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let layout = layout::compute(area, state);

    render_header(frame, layout.header);
    render_tree(frame, state, &layout);
    render_detail(frame, state, &layout);
    if let (Some(output), Some(inner)) = (layout.output, layout.output_inner) {
        render_output(frame, state, output, inner);
    }
    render_status(frame, state, layout.status);
    if state.show_help {
        render_help(frame, area);
    }
}

fn pane_block(icon: &str, title: &str, focused: bool) -> Block<'static> {
    let border = if focused { THEME.primary } else { THEME.border };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(Line::from(vec![
            Span::styled(format!(" {icon} "), Style::new().fg(THEME.dim)),
            Span::styled(format!("{title} "), Style::new().fg(THEME.accent).bold()),
        ]))
}

/// `── Title ─────` divider spanning `width` cells.
fn divider(title: &str, width: u16) -> Line<'static> {
    let prefix = format!("── {title} ");
    let used = prefix.chars().count();
    let rest = usize::from(width).saturating_sub(used);
    Line::from(vec![
        Span::styled(prefix, Style::new().fg(THEME.dim)),
        Span::styled("─".repeat(rest), Style::new().fg(THEME.faint)),
    ])
}

fn spinner(state: &AppState) -> &'static str {
    SPINNER[(state.frame % SPINNER.len() as u64) as usize]
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

// ─── Header ────────────────────────────────────────────────────────────────

fn render_header(frame: &mut Frame, area: Rect) {
    let bar = Style::new().bg(THEME.bar_bg);
    frame.render_widget(Block::new().style(bar), area);
    let left = Line::from(vec![
        Span::styled(" ⬡", Style::new().fg(THEME.accent).bold()),
        Span::styled(" makit ", Style::new().fg(THEME.primary).bold()),
    ]);
    frame.render_widget(Paragraph::new(left), area);
    let hints = Line::from("e·execute  Tab·focus  o·output  ?·help  q·quit ").fg(THEME.dim);
    frame.render_widget(Paragraph::new(hints).alignment(Alignment::Right), area);
}

// ─── Explorer ──────────────────────────────────────────────────────────────

fn render_tree(frame: &mut Frame, state: &AppState, layout: &AppLayout) {
    let focused = state.focus == Focus::Tree;
    let items: Vec<ListItem> = state
        .tree
        .rows()
        .iter()
        .map(|row| {
            let marker = match (row.has_children, row.expanded) {
                (true, true) => "▾ ",
                (true, false) => "▹ ",
                (false, _) => "  ",
            };
            let label_style = if row.depth == 0 {
                Style::new().fg(THEME.secondary).bold()
            } else if row.id.contains(":opt:") {
                Style::new().fg(THEME.dim)
            } else {
                Style::new().fg(THEME.text)
            };
            ListItem::new(Line::from(vec![
                Span::raw("  ".repeat(row.depth)),
                Span::styled(marker, Style::new().fg(THEME.dim)),
                Span::styled(row.label.clone(), label_style),
            ]))
        })
        .collect();

    let highlight = if focused {
        Style::new()
            .bg(THEME.selection_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().bg(THEME.bar_bg)
    };
    let list = List::new(items)
        .block(pane_block("⊟", "Explorer", focused))
        .highlight_style(highlight);
    let mut list_state = ListState::default()
        .with_offset(state.tree.offset())
        .with_selected((!state.tree.rows().is_empty()).then(|| state.tree.selected()));
    frame.render_stateful_widget(list, layout.tree, &mut list_state);
}

// ─── Detail ────────────────────────────────────────────────────────────────

fn render_detail(frame: &mut Frame, state: &AppState, layout: &AppLayout) {
    frame.render_widget(pane_block("◈", "Detail", false), layout.detail);
    render_info(frame, state, layout.info);
    if let Some(ifc) = &layout.ifc {
        render_ifc(frame, state, ifc);
    }

    let kind = state.preview_kind();
    frame.render_widget(
        Paragraph::new(divider(
            &format!("Preview · {}", preview_title(kind)),
            layout.preview_label.width,
        )),
        layout.preview_label,
    );
    render_preview(frame, state, kind, layout.preview);
    let caption: Vec<Line> = preview::caption(kind, state.murb_results.as_ref())
        .into_iter()
        .map(|l| Line::from(l).fg(THEME.accent))
        .collect();
    frame.render_widget(Paragraph::new(caption), layout.caption);
}

fn preview_title(kind: PreviewKind) -> &'static str {
    match kind {
        PreviewKind::Energy => "heating demand",
        PreviewKind::Walls => "wall orientation",
        PreviewKind::Building => "isometric",
        PreviewKind::FloorPlan => "floor plan",
        PreviewKind::Logo => "makit",
    }
}

fn render_info(frame: &mut Frame, state: &AppState, area: Rect) {
    let active = state.active_node();
    let focus_name = active
        .as_ref()
        .map_or("—", |n| n.display_name())
        .to_string();
    let open_name = state
        .opened
        .as_ref()
        .map_or("—", |n| n.display_name())
        .to_string();

    let mut first = vec![
        Span::styled("Focus: ", Style::new().fg(THEME.dim)),
        Span::styled(focus_name, Style::new().fg(THEME.text).bold()),
        Span::styled("   Open: ", Style::new().fg(THEME.dim)),
        Span::styled(open_name, Style::new().fg(THEME.dim)),
    ];
    if active.as_ref().is_some_and(|n| n.is_executable()) {
        first.push(Span::styled("   e·execute", Style::new().fg(THEME.faint)));
    }
    let lines = vec![
        Line::from(first),
        Line::from(state.active_description().to_string())
            .fg(THEME.dim)
            .italic(),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_ifc(frame: &mut Frame, state: &AppState, ifc: &IfcLayout) {
    frame.render_widget(
        Paragraph::new(divider("IFC file", ifc.label.width)),
        ifc.label,
    );

    let Some(list_area) = ifc.list else {
        frame.render_widget(
            Paragraph::new("No .ifc files found in working directory").fg(THEME.dim),
            ifc.details,
        );
        return;
    };

    let focused = state.focus == Focus::IfcPicker;
    let items: Vec<ListItem> = state
        .ifc
        .files
        .iter()
        .map(|path| {
            let mark = if state.ifc.loading.as_deref() == Some(path.as_str()) {
                Span::styled(
                    format!("{} ", spinner(state)),
                    Style::new().fg(THEME.primary),
                )
            } else if state.ifc.loaded.as_deref() == Some(path.as_str()) {
                Span::styled("✓ ", Style::new().fg(THEME.success))
            } else {
                Span::raw("  ")
            };
            ListItem::new(Line::from(vec![
                mark,
                Span::styled(file_name(path).to_string(), Style::new().fg(THEME.text)),
            ]))
        })
        .collect();
    let highlight = if focused {
        Style::new()
            .bg(THEME.selection_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    let border = if focused { THEME.primary } else { THEME.border };
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border))
                .title(Span::styled(" Enter·load ", Style::new().fg(THEME.faint))),
        )
        .highlight_style(highlight);
    let mut list_state = ListState::default()
        .with_offset(state.ifc.offset)
        .with_selected(Some(state.ifc.selected));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    render_ifc_details(frame, state, ifc.details);
}

fn render_ifc_details(frame: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 {
        return;
    }
    if let Some(analysis) = &state.ifc.analysis {
        let mut y = area.y;
        if !analysis.project_name.is_empty() {
            let line = Line::from(vec![
                Span::styled("Project: ", Style::new().fg(THEME.dim)),
                Span::styled(
                    analysis.project_name.clone(),
                    Style::new().fg(THEME.text).bold(),
                ),
            ]);
            frame.render_widget(
                Paragraph::new(line),
                Rect {
                    y,
                    height: 1,
                    ..area
                },
            );
            y += 1;
        }
        let rows: Vec<Row> = analysis
            .orientations
            .iter()
            .map(|r| {
                Row::new(vec![
                    r.direction.clone(),
                    r.count.to_string(),
                    format!("{:.1}", r.total_area_sqm),
                    format!("{:.1}%", r.percentage),
                ])
                .style(Style::new().fg(THEME.accent))
            })
            .collect();
        let table_h = analysis.orientations.len() as u16 + 1;
        let table = Table::new(
            rows,
            [
                Constraint::Length(7),
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Length(7),
            ],
        )
        .header(
            Row::new(vec!["Dir", "Walls", "Area m²", "Share"]).style(Style::new().fg(THEME.dim)),
        );
        let bottom = area.y + area.height;
        let table_area = Rect {
            y,
            height: table_h.min(bottom.saturating_sub(y)),
            ..area
        };
        frame.render_widget(table, table_area);
        y += table_area.height;
        if y < bottom {
            let summary = Line::from(vec![
                Span::styled(
                    format!(
                        "Walls {} ({:.1} m²) · Windows {} ({:.1} m²) · ",
                        analysis.wall_count,
                        analysis.total_wall_area,
                        analysis.window_count,
                        analysis.total_window_area
                    ),
                    Style::new().fg(THEME.text),
                ),
                Span::styled(
                    format!("WWR {:.1}%", analysis.overall_wwr),
                    Style::new().fg(THEME.primary).bold(),
                ),
            ]);
            frame.render_widget(
                Paragraph::new(summary),
                Rect {
                    y,
                    height: 1,
                    ..area
                },
            );
        }
    } else if let Some(loading) = &state.ifc.loading {
        let line = Line::from(format!(
            "{} Loading {}…",
            spinner(state),
            file_name(loading)
        ))
        .fg(THEME.dim);
        frame.render_widget(Paragraph::new(line), area);
    } else if let Some(loaded) = &state.ifc.loaded {
        let lines = vec![
            Line::from(vec![
                Span::styled("✓ ", Style::new().fg(THEME.success)),
                Span::styled(file_name(loaded).to_string(), Style::new().fg(THEME.text)),
            ]),
            Line::from("Extract JSON for analysis results").fg(THEME.dim),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn render_preview(frame: &mut Frame, state: &AppState, kind: PreviewKind, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rec = preview::record(kind, state.logo_angle, state.murb_results.as_ref());
    let Some(bounds) = preview::content_bounds(kind, &rec) else {
        return;
    };
    let fit = preview::fit_bounds(bounds, area.width, area.height);
    let points = preview::expand_points(&rec.points, fit.scale);
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds(fit.x_bounds)
        .y_bounds(fit.y_bounds)
        .paint(|ctx| {
            for &((x1, y1), (x2, y2)) in &rec.segments {
                ctx.draw(&CanvasLine::new(x1, y1, x2, y2, THEME.accent));
            }
            ctx.draw(&Points {
                coords: &points,
                color: THEME.accent,
            });
            ctx.layer();
            for (x, y, text) in &rec.labels {
                ctx.print(
                    *x,
                    *y,
                    Span::styled(text.clone(), Style::new().fg(THEME.dim)),
                );
            }
        });
    frame.render_widget(canvas, area);
}

// ─── Output ────────────────────────────────────────────────────────────────

fn render_output(frame: &mut Frame, state: &AppState, area: Rect, inner: Rect) {
    let focused = state.focus == Focus::Output;
    let mut title = format!("Output ({})", state.log.len());
    if state.log_scroll > 0 {
        title.push_str(&format!(" ↑{}", state.log_scroll));
    }
    let block = pane_block("▤", &title, focused);
    let rows = usize::from(inner.height);
    let end = state.log.len().saturating_sub(state.log_scroll);
    let start = end.saturating_sub(rows);
    let lines: Vec<Line> = state
        .log
        .range(start..end)
        .map(|l| {
            if l.starts_with("── ") {
                Line::from(l.clone()).fg(THEME.secondary)
            } else {
                Line::from(l.clone()).fg(THEME.text)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ─── Status bar ────────────────────────────────────────────────────────────

fn render_status(frame: &mut Frame, state: &AppState, area: Rect) {
    frame.render_widget(Block::new().style(Style::new().bg(THEME.bar_bg)), area);
    let status_color = if state.status.starts_with('✗') {
        THEME.error
    } else if state.status.starts_with('✓') {
        THEME.success
    } else {
        THEME.dim
    };
    frame.render_widget(
        Paragraph::new(Line::from(format!(" {}", state.status)).fg(status_color)),
        area,
    );

    let busy = state.tasks.len() + usize::from(state.ifc.loading.is_some());
    let mut right = Vec::new();
    if busy > 0 {
        right.push(Span::styled(
            format!("{} {busy} running · ", spinner(state)),
            Style::new().fg(THEME.primary),
        ));
    }
    right.push(Span::styled(
        format!("makit v{}", env!("CARGO_PKG_VERSION")),
        Style::new().fg(THEME.faint),
    ));
    right.push(Span::styled(" · ratatui ", Style::new().fg(THEME.faint)));
    frame.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        area,
    );
}

// ─── Help overlay ──────────────────────────────────────────────────────────

fn render_help(frame: &mut Frame, area: Rect) {
    let popup = layout::centered(area, HELP_WIDTH, HELP_HEIGHT);
    frame.render_widget(Clear, popup);

    let key = |k: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {k:<11}"), Style::new().fg(THEME.primary).bold()),
            Span::styled(desc.to_string(), Style::new().fg(THEME.text)),
        ])
    };
    let legend = |icon: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {icon}  "), Style::new().fg(THEME.accent)),
            Span::styled(desc.to_string(), Style::new().fg(THEME.text)),
        ])
    };
    let inner_width = popup.width.saturating_sub(4);
    let lines = vec![
        Line::default(),
        key("↑ ↓  j k", "Move selection"),
        key("PgUp PgDn", "Page up / down"),
        key("Home End", "First / last item"),
        key("→  l", "Expand node"),
        key("←  h", "Collapse node / go to parent"),
        key("⏎", "Open node · load highlighted IFC file"),
        key("e", "Execute focused source or action"),
        key("Tab", "Cycle focus: Explorer → IFC files → Output"),
        key("o", "Toggle the Output pane"),
        key("c", "Clear Output (when focused)"),
        key("?", "Toggle this help"),
        key("Esc  q", "Close help · quit"),
        key("Mouse", "Wheel scrolls · click selects / opens"),
        Line::default(),
        divider("Legend", inner_width),
        legend("◆", "Sources — geometry input drivers"),
        legend("▸", "Actions — operations on geometry"),
        legend("⊞", "Groups — analysis · extraction · reporting"),
        Line::default(),
        Line::from("Esc or ? to close")
            .fg(THEME.dim)
            .alignment(Alignment::Center),
    ];
    let block = pane_block("◈", "Help", true).style(Style::new().bg(THEME.bar_bg));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{InitialData, RunningTask};
    use crate::tree::TreeNode;
    use makit_tools::building_model;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn state(w: u16, h: u16) -> AppState {
        AppState::new(InitialData {
            tree: vec![
                TreeNode::new("sources", "Sources")
                    .child(TreeNode::new("source:ifc", "◆ ifc").description("IFC file loader"))
                    .child(
                        TreeNode::new("source:revit", "◆ revit")
                            .description("Autodesk Revit integration"),
                    ),
                TreeNode::new("actions", "Actions").child(
                    TreeNode::new("cat:energy", "⊞ energy")
                        .child(TreeNode::new("action:murb-simulate", "▸ murb-simulate")),
                ),
            ],
            ifc_files: vec!["examples/IFC/Building-Architecture.ifc".into()],
            murb_results: None,
            viewport: Rect::new(0, 0, w, h),
        })
    }

    fn render(state: &AppState) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(
            state.viewport.width,
            state.viewport.height,
        ))
        .unwrap();
        terminal.draw(|f| view(f, state)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn text(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    fn row_of(lines: &[String], needle: &str) -> usize {
        lines
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} not on screen:\n{}", lines.join("\n")))
    }

    fn has_braille(lines: &[String]) -> bool {
        lines
            .iter()
            .flat_map(|l| l.chars())
            .any(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
    }

    #[test]
    fn wide_layout_renders_all_regions() {
        let s = state(120, 40);
        let lines = text(&render(&s));
        assert!(lines[0].contains("makit"));
        assert!(lines[0].contains("?·help"));
        let explorer = row_of(&lines, "Explorer");
        assert_eq!(
            explorer,
            row_of(&lines, "Detail"),
            "panes should sit side by side"
        );
        assert!(lines.iter().any(|l| l.contains("▾ Sources")));
        assert!(lines.iter().any(|l| l.contains("◆ revit")));
        assert!(lines.iter().any(|l| l.contains("Preview · makit")));
        assert!(lines[39].contains("Ready"));
        assert!(lines[39].contains("ratatui"));
        assert!(has_braille(&lines), "logo preview should draw braille dots");
    }

    #[test]
    fn narrow_layout_stacks_explorer_above_detail() {
        let s = state(70, 40);
        let lines = text(&render(&s));
        assert!(row_of(&lines, "Explorer") < row_of(&lines, "Detail"));
    }

    #[test]
    fn help_overlay_lists_keys() {
        let mut s = state(120, 40);
        s.show_help = true;
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("Execute focused source or action")));
        assert!(lines
            .iter()
            .any(|l| l.contains("Sources — geometry input drivers")));
    }

    #[test]
    fn ifc_picker_shows_files_and_analysis_table() {
        let mut s = state(120, 40);
        s.tree.select(1); // source:ifc
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/building-model.json"
        );
        let analysis =
            building_model::analyze_wwr(&building_model::load_model(Path::new(path)).unwrap());
        let first_dir = analysis.orientations[0].direction.clone();
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        s.ifc.analysis = Some(analysis);
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("IFC file")));
        assert!(lines
            .iter()
            .any(|l| l.contains("✓ Building-Architecture.ifc")));
        assert!(lines.iter().any(|l| l.contains("Area m²")));
        assert!(lines.iter().any(|l| l.contains(&first_dir)));
        assert!(lines.iter().any(|l| l.contains("WWR")));
        assert!(lines.iter().any(|l| l.contains("Focus: ifc")));
        assert!(lines.iter().any(|l| l.contains("IFC file loader")));
    }

    #[test]
    fn energy_preview_shows_month_labels_and_caption() {
        let mut s = state(120, 40);
        s.tree.select(4); // cat:energy — expand it to reach the action
        s.tree.toggle();
        s.tree.select(5);
        assert_eq!(s.tree.selected_id(), Some("action:murb-simulate"));
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("Preview · heating demand")));
        assert!(lines
            .iter()
            .any(|l| l.contains("No results — run murb-simulate")));
        assert!(has_braille(&lines));
    }

    #[test]
    fn output_pane_shows_latest_lines() {
        let mut s = state(120, 40);
        s.show_output = true;
        for i in 0..30 {
            s.push_log(format!("log line {i}"));
        }
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("Output (30)")));
        assert!(lines.iter().any(|l| l.contains("log line 29")));
        assert!(!lines.iter().any(|l| l.contains("log line 10 ")));
    }

    #[test]
    fn status_bar_shows_running_tasks() {
        let mut s = state(120, 40);
        s.tasks.insert(
            1,
            RunningTask {
                name: "revit".into(),
            },
        );
        s.status = "✗ Error: revit: connection refused".into();
        let lines = text(&render(&s));
        assert!(lines[39].contains("1 running"));
        assert!(lines[39].contains("✗ Error: revit"));
    }

    #[test]
    fn tiny_terminal_renders_without_panicking() {
        for (w, h) in [(1, 1), (20, 5), (79, 12)] {
            let mut s = state(w, h);
            s.show_output = true;
            s.show_help = true;
            s.tree.select(1);
            render(&s);
        }
    }
}
