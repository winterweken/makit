//! `view(frame, state)` — render the whole UI from state. No mutation.

use std::path::Path;

use ratatui::layout::{Alignment, Constraint, Margin, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Row, Table,
};
use ratatui::Frame;

use crate::app::{AppState, Editor, Focus, ValueSource};
use crate::help::{self, HelpLine};
use crate::layout::{self, AppLayout, IfcLayout};
use crate::logo;
use crate::node::{NodeId, PreviewKind};
use crate::options::OptionSpec;
use crate::preview;
use crate::theme::THEME;
use makit_tools::building_model;

/// Braille spinner frames for running work.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Columns kept clear between the status text and the right-hand label.
const STATUS_GAP: usize = 2;

/// Header key hints, longest first; the first that fits is shown.
const HEADER_HINTS: [&str; 3] = [
    "⏎·open/edit  e·execute  x·stop  Tab·focus  o·output  ?·help  q·quit ",
    "e·execute  x·stop  ?·help  q·quit ",
    "?·help  q·quit ",
];
const HEADER_TITLE: &str = " makit ";

/// Prompt drawn before the editor's text.
const EDITOR_PROMPT: &str = "› ";

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
        render_help(frame, state, area);
    }
    if let Some(editor) = &state.editor {
        render_editor(frame, state, editor, area);
    }
    if let Some(position) = cursor_position(state, area) {
        frame.set_cursor_position(position);
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

/// `text` cut to `max` columns, ending in `…` when anything was cut.
fn truncate(text: &str, max: usize) -> String {
    if char_len(text) <= max {
        text.to_string()
    } else if max == 0 {
        String::new()
    } else {
        text.chars().take(max - 1).chain(['…']).collect()
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
    let [cube_l, cube_r] = logo::header_icon();
    let left = Line::from(vec![
        Span::raw(" "),
        cube_l,
        cube_r,
        Span::styled(HEADER_TITLE, Style::new().fg(THEME.primary).bold()),
    ]);
    let title_width = left.width();
    frame.render_widget(Paragraph::new(left), area);
    // Hints are a courtesy: never let them run into the title.
    let room = usize::from(area.width).saturating_sub(title_width + 1);
    if let Some(hints) = HEADER_HINTS.iter().find(|h| char_len(h) <= room) {
        let hints = Line::from(*hints).fg(THEME.dim);
        frame.render_widget(Paragraph::new(hints).alignment(Alignment::Right), area);
    }
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
            let mut spans = vec![
                Span::raw("  ".repeat(row.depth)),
                Span::styled(marker, Style::new().fg(THEME.dim)),
            ];
            match set_value(state, &row.id) {
                // An option with a value shows it in place of its description.
                Some((opt, value)) => {
                    spans.push(Span::styled(format!("--{opt} = "), label_style));
                    spans.push(Span::styled(value, Style::new().fg(THEME.primary)));
                }
                None => spans.push(Span::styled(row.label.clone(), label_style)),
            }
            ListItem::new(Line::from(spans))
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

/// The option name and value of tree row `id`, if it is an option whose
/// value the user typed or picked (defaults are part of the row's label).
fn set_value(state: &AppState, id: &str) -> Option<(String, String)> {
    let node = NodeId::parse(id)?;
    let opt = node.opt_name()?;
    match state.option_value(&node.task_node()?, opt)? {
        (value, ValueSource::User | ValueSource::Picker) => Some((opt.to_string(), value)),
        (_, ValueSource::Default) => None,
    }
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
            &format!("Preview · {}", preview::title(kind, state.preview_data())),
            layout.preview_label.width,
        )),
        layout.preview_label,
    );
    render_preview(frame, state, kind, layout.preview);
    let caption: Vec<Line> = preview::caption(kind, state.preview_data())
        .into_iter()
        .map(|l| Line::from(l).fg(THEME.accent))
        .collect();
    frame.render_widget(Paragraph::new(caption), layout.caption);
}

fn render_info(frame: &mut Frame, state: &AppState, area: Rect) {
    let active = state.active_node();
    let focus_name = active.as_ref().map_or("—".to_string(), NodeId::label);
    let open_name = state
        .opened
        .as_ref()
        .map_or("—", |n| n.display_name())
        .to_string();
    let option = active.as_ref().and_then(|node| {
        let opt = node.opt_name()?;
        Some((node.task_node()?, opt.to_string()))
    });
    let spec = option
        .as_ref()
        .and_then(|(task, opt)| state.catalog.spec(task, opt));

    let mut first = vec![
        Span::styled("Focus: ", Style::new().fg(THEME.dim)),
        Span::styled(focus_name, Style::new().fg(THEME.text).bold()),
        Span::styled("   Open: ", Style::new().fg(THEME.dim)),
        Span::styled(open_name, Style::new().fg(THEME.dim)),
    ];
    let mut hints = Vec::new();
    if option.is_some() {
        hints.push(if spec.is_some_and(OptionSpec::is_flag) {
            "⏎·toggle"
        } else {
            "⏎·edit"
        });
    }
    if active.as_ref().is_some_and(|n| n.is_executable()) {
        hints.push("e·execute");
    }
    if !hints.is_empty() {
        first.push(Span::styled(
            format!("   {}", hints.join("  ")),
            Style::new().fg(THEME.faint),
        ));
    }
    let mut lines = vec![
        Line::from(first),
        Line::from(state.active_description().to_string())
            .fg(THEME.dim)
            .italic(),
    ];
    if let Some((task, opt)) = &option {
        lines.push(option_line(state, task, opt, spec));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Type, requirement and current value of the focused option.
fn option_line(
    state: &AppState,
    task: &NodeId,
    opt: &str,
    spec: Option<&OptionSpec>,
) -> Line<'static> {
    let mut spans = Vec::new();
    if let Some(spec) = spec {
        spans.push(Span::styled(
            format!("{}   ", spec.summary()),
            Style::new().fg(THEME.dim),
        ));
    }
    match state.option_value(task, opt) {
        Some((value, source)) => {
            let origin = match source {
                ValueSource::User => "typed",
                ValueSource::Picker => "from the IFC picker",
                ValueSource::Default => "default",
            };
            spans.push(Span::styled("= ", Style::new().fg(THEME.dim)));
            spans.push(Span::styled(value, Style::new().fg(THEME.primary).bold()));
            spans.push(Span::styled(
                format!(" ({origin})"),
                Style::new().fg(THEME.dim),
            ));
        }
        None => spans.push(Span::styled("not set", Style::new().fg(THEME.faint))),
    }
    Line::from(spans)
}

fn render_ifc(frame: &mut Frame, state: &AppState, ifc: &IfcLayout) {
    frame.render_widget(
        Paragraph::new(divider("IFC file", ifc.label.width)),
        ifc.label,
    );

    let Some(list_area) = ifc.list else {
        if state.ifc.files.is_empty() {
            frame.render_widget(
                Paragraph::new("No .ifc files found in working directory").fg(THEME.dim),
                ifc.details,
            );
        } else {
            // Files exist, but the screen is too short to list them.
            render_ifc_details(frame, state, ifc.details);
        }
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
                if state.ifc.error.is_some() {
                    Span::styled("✗ ", Style::new().fg(THEME.error))
                } else {
                    Span::styled("✓ ", Style::new().fg(THEME.success))
                }
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
            y += 1;
        }
        if let Some(env) = state.ifc.envelope.as_ref().filter(|_| y < bottom) {
            let line = Line::from(format!(
                "GFA {:.0} m² · walls {:.0} / {:.0} m² · windows {:.0} m² · roof {:.0} m²",
                env.gfa, env.walls_ag, env.walls_bg, env.windows, env.roof
            ))
            .fg(THEME.text);
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
        if let Some(note) = state.ifc.note.as_ref().filter(|_| y < bottom) {
            frame.render_widget(
                Paragraph::new(Line::from(note.clone()).fg(THEME.dim)),
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
        let name = Span::styled(file_name(loaded).to_string(), Style::new().fg(THEME.text));
        let lines = match &state.ifc.error {
            Some(error) => vec![
                Line::from(vec![Span::styled("✗ ", Style::new().fg(THEME.error)), name]),
                Line::from(error.clone()).fg(THEME.error),
            ],
            None => {
                let json = building_model::extraction_path(Path::new(loaded));
                let why = state.ifc.note.clone().unwrap_or_else(|| {
                    format!(
                        "Not extracted yet: no {} beside it",
                        file_name(&json.to_string_lossy())
                    )
                });
                vec![
                    Line::from(vec![Span::styled("◆ ", Style::new().fg(THEME.dim)), name]),
                    Line::from(why).fg(THEME.dim),
                ]
            }
        };
        frame.render_widget(Paragraph::new(lines), area);
    }
}

fn render_preview(frame: &mut Frame, state: &AppState, kind: PreviewKind, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rec = preview::record(kind, state.logo_angle, state.preview_data());
    let Some(bounds) = preview::content_bounds(kind, &rec) else {
        return;
    };
    let fit = preview::fit_bounds(bounds, area.width, area.height);
    let points = preview::expand_points(&rec.points, fit.scale);
    // The logo is drawn in colour: the kit in slate, the lifted block in the
    // theme's three colours. Other previews are one colour.
    let toned = (kind == PreviewKind::Logo).then(|| logo::segments(state.logo_angle));
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds(fit.x_bounds)
        .y_bounds(fit.y_bounds)
        .paint(|ctx| {
            if let Some(toned) = &toned {
                // Kit first, so the hero block's edges win where they meet.
                for &(((x1, y1), (x2, y2)), tone) in
                    toned.iter().filter(|(_, t)| *t == logo::Tone::Kit)
                {
                    ctx.draw(&CanvasLine::new(x1, y1, x2, y2, tone.color()));
                }
                ctx.layer();
                for &(((x1, y1), (x2, y2)), tone) in
                    toned.iter().filter(|(_, t)| *t != logo::Tone::Kit)
                {
                    ctx.draw(&CanvasLine::new(x1, y1, x2, y2, tone.color()));
                }
            } else {
                for &((x1, y1), (x2, y2)) in &rec.segments {
                    ctx.draw(&CanvasLine::new(x1, y1, x2, y2, THEME.accent));
                }
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
    let busy = state.tasks.len() + usize::from(state.ifc.loading.is_some());
    let busy = (busy > 0).then(|| format!("{} {busy} running", spinner(state)));
    let (left, right) = status_bar(&state.status, busy.as_deref(), area.width);

    frame.render_widget(Paragraph::new(Line::from(left).fg(status_color)), area);
    let right = match busy.as_deref().and_then(|b| right.strip_prefix(b)) {
        Some(rest) => vec![
            Span::styled(
                busy.clone().unwrap_or_default(),
                Style::new().fg(THEME.primary),
            ),
            Span::styled(rest.to_string(), Style::new().fg(THEME.faint)),
        ],
        None => vec![Span::styled(right, Style::new().fg(THEME.faint))],
    };
    frame.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        area,
    );
}

/// Split the status bar's `width` columns between the status text (left)
/// and the running count plus version label (right) so they never overlap.
///
/// The status matters most: the version label goes first, then the status
/// is cut with `…`; the running count stays as long as it leaves room.
fn status_bar(status: &str, busy: Option<&str>, width: u16) -> (String, String) {
    let width = usize::from(width);
    let left = format!(" {status}");
    let version = format!("makit v{} · ratatui ", env!("CARGO_PKG_VERSION"));
    let full = match busy {
        Some(busy) => format!("{busy} · {version}"),
        None => version,
    };
    let short = busy.map(|busy| format!("{busy} ")).unwrap_or_default();
    // Columns a right-hand label takes, with the gap that sets it apart.
    let taken = |right: &str| match char_len(right) {
        0 => 0,
        len => len + STATUS_GAP,
    };

    if char_len(&left) + taken(&full) <= width {
        return (left, full);
    }
    let right = if taken(&short) < width {
        short
    } else {
        String::new()
    };
    let left = truncate(&left, width - taken(&right));
    (left, right)
}

// ─── Help overlay ──────────────────────────────────────────────────────────

fn render_help(frame: &mut Frame, state: &AppState, area: Rect) {
    let popup = layout::help_rect(area);
    frame.render_widget(Clear, popup);

    let inner_width = popup.width.saturating_sub(4);
    let lines: Vec<Line> = help::LINES
        .iter()
        .map(|line| match *line {
            HelpLine::Blank => Line::default(),
            HelpLine::Key(key, what) => Line::from(vec![
                Span::styled(
                    format!("  {key:<11}"),
                    Style::new().fg(THEME.primary).bold(),
                ),
                Span::styled(what, Style::new().fg(THEME.text)),
            ]),
            HelpLine::Section(title) => divider(title, inner_width),
            HelpLine::Legend(icon, what) => Line::from(vec![
                Span::styled(format!("  {icon}  "), Style::new().fg(THEME.accent)),
                Span::styled(what, Style::new().fg(THEME.text)),
            ]),
        })
        .collect();

    let max_scroll = layout::help_max_scroll(area);
    let scroll = state.help_scroll.min(max_scroll);
    let hint = if max_scroll > 0 {
        format!(
            " ↑↓ scroll · Esc close · {}/{} ",
            scroll + 1,
            max_scroll + 1
        )
    } else {
        " Esc or ? to close ".to_string()
    };
    let block = pane_block("◈", "Help", true)
        .style(Style::new().bg(THEME.bar_bg))
        .title_bottom(Line::from(hint).fg(THEME.dim).centered());
    frame.render_widget(
        Paragraph::new(lines).block(block).scroll((scroll, 0)),
        popup,
    );
}

// ─── Option editor ─────────────────────────────────────────────────────────

/// The text area inside the editor form, and how many of its rows come
/// before the input row. Short screens drop the explanation lines first so
/// the field itself is always there to type into.
fn editor_rows(area: Rect) -> (Rect, u16) {
    let inner = layout::bordered_inner(layout::editor_rect(area)).inner(Margin::new(1, 0));
    let before = match inner.height {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    };
    (inner, before)
}

/// The part of the editor's text that fits in `width` columns with the
/// cursor in view, and the cursor's column within it.
fn editor_window(editor: &Editor, width: u16) -> (String, u16) {
    let width = usize::from(width);
    // One column stays free for the cursor after the last character.
    let start = (editor.cursor + 1).saturating_sub(width);
    let visible = editor.buffer.chars().skip(start).take(width).collect();
    (visible, (editor.cursor - start) as u16)
}

/// Where the terminal's text cursor belongs: in the open editor's field.
/// `None` leaves it hidden.
pub fn cursor_position(state: &AppState, area: Rect) -> Option<Position> {
    let editor = state.editor.as_ref()?;
    let (inner, before) = editor_rows(area);
    let prompt = EDITOR_PROMPT.chars().count() as u16;
    let width = inner.width.checked_sub(prompt).filter(|w| *w > 0)?;
    if inner.height == 0 {
        return None;
    }
    let (_, column) = editor_window(editor, width);
    Some(Position::new(inner.x + prompt + column, inner.y + before))
}

fn render_editor(frame: &mut Frame, state: &AppState, editor: &Editor, area: Rect) {
    let popup = layout::editor_rect(area);
    frame.render_widget(Clear, popup);
    let label = format!("{} --{}", editor.task.display_name(), editor.opt);
    let block = pane_block("✎", &label, true).style(Style::new().bg(THEME.bar_bg));
    frame.render_widget(block, popup);

    let (inner, before) = editor_rows(area);
    if inner.height == 0 {
        return;
    }
    let spec = state.catalog.spec(&editor.task, &editor.opt);
    let mut summary = spec.map(OptionSpec::summary).unwrap_or_default();
    if editor.run_after {
        // This option is one of the missing ones; say how many follow it.
        let after = state.missing_required(&editor.task).len().saturating_sub(1);
        if after > 0 {
            summary.push_str(&format!(" · {after} more required after this"));
        }
    }
    let description = spec.map_or("", |s| s.description.as_str()).to_string();
    let explanation = [
        Line::from(description).fg(THEME.dim).italic(),
        Line::from(summary).fg(THEME.dim),
        Line::default(),
    ];

    let prompt = EDITOR_PROMPT.chars().count() as u16;
    let (visible, _) = editor_window(editor, inner.width.saturating_sub(prompt));
    let accept = if editor.run_after {
        "⏎ run"
    } else {
        "⏎ save"
    };
    let mut lines: Vec<Line> = explanation.into_iter().take(usize::from(before)).collect();
    lines.push(Line::from(vec![
        Span::styled(EDITOR_PROMPT, Style::new().fg(THEME.primary).bold()),
        Span::styled(visible, Style::new().fg(THEME.text)),
    ]));
    lines.push(Line::from(format!("{accept} · Esc cancel · Ctrl-U clear")).fg(THEME.faint));
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Editor, InitialData, RunningTask};
    use crate::node::NodeId;
    use crate::options::OptionSpec;
    use crate::tree::TreeNode;
    use makit_tools::building_model;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Position;
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
            ..InitialData::default()
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
    fn after_output_opens_a_click_lands_on_the_row_drawn_there() {
        use crate::msg::{Msg, Pane};
        use crate::update::update;
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

        // Five rows fit at 120×10 until the Output pane opens.
        let mut s = state(120, 10);
        update(&mut s, Msg::End);
        let last = s.tree.selected();
        assert_eq!(s.tree.rows()[last].id, "cat:energy");
        update(&mut s, Msg::ToggleOutput);

        let lines = text(&render(&s));
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: row_of(&lines, "⊞ energy") as u16,
            modifiers: KeyModifiers::NONE,
        };
        let msg = crate::keymap::map_mouse(&s, &click);
        assert!(
            matches!(
                msg,
                Some(Msg::Click {
                    pane: Pane::Tree,
                    row: Some(row),
                }) if row == last
            ),
            "the click gave {msg:?}, wanted row {last}; screen:\n{}",
            lines.join("\n")
        );
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
        assert!(lines.iter().any(|l| l.contains("Run source or action")));
        assert!(lines.iter().any(|l| l.contains("Stop the running task")));
        assert!(lines
            .iter()
            .any(|l| l.contains("Sources — geometry input drivers")));
    }

    #[test]
    fn a_short_screen_shows_no_smeared_preview_or_empty_picker_box() {
        let mut s = state(120, 8);
        s.tree.select(1); // source:ifc
        let lines = text(&render(&s));
        let screen = lines.join("\n");
        assert!(screen.contains("IFC file"));
        // No room for a list row: no box, and no claim that nothing was found.
        assert!(!screen.contains("Enter·load"), "{screen}");
        assert!(!screen.contains("No .ifc files"), "{screen}");
        // No room for a readable canvas: no heading for one either.
        assert!(!screen.contains("Preview"), "{screen}");
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
        assert!(lines
            .iter()
            .any(|l| l.contains("Preview · heating demand · no results")));
        assert!(lines
            .iter()
            .any(|l| l.contains("No results — run murb-simulate")));
        // Month labels sit under an empty chart.
        assert!(lines.iter().any(|l| l.contains('J') && l.contains('D')));
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
        s.tasks
            .insert(1, RunningTask::new(NodeId::Source("revit".into())));
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

    // ─── Status bar and header ──────────────────────────────────────────

    fn width(s: &str) -> usize {
        s.chars().count()
    }

    #[test]
    fn status_bar_shows_everything_when_there_is_room() {
        let (left, right) = status_bar("Ready", None, 120);
        assert_eq!(left, " Ready");
        assert!(right.starts_with("makit v") && right.ends_with("· ratatui "));
        let (_, right) = status_bar("Ready", Some("⠋ 1 running"), 120);
        assert!(right.starts_with("⠋ 1 running · makit v"), "{right}");
    }

    #[test]
    fn status_bar_drops_the_version_before_touching_the_status() {
        let status = "✗ Error: revit: connection refused (os error 61)";
        let (left, right) = status_bar(status, None, 60);
        assert_eq!(left, format!(" {status}"));
        assert_eq!(right, "");

        let (left, right) = status_bar(status, Some("⠋ 2 running"), 66);
        assert_eq!(left, format!(" {status}"));
        assert_eq!(right, "⠋ 2 running ");
    }

    #[test]
    fn status_bar_truncates_a_status_that_cannot_fit() {
        let status = "✗ Error loading examples/IFC/Building-Architecture.ifc: missing field";
        let (left, right) = status_bar(status, None, 30);
        assert_eq!(width(&left), 30);
        assert!(left.starts_with(" ✗ Error loading") && left.ends_with('…'));
        assert_eq!(right, "");

        // The running count stays, with a gap before it.
        let (left, right) = status_bar(status, Some("⠋ 1 running"), 40);
        assert_eq!(right, "⠋ 1 running ");
        assert!(left.ends_with('…'));
        assert_eq!(width(&left) + STATUS_GAP + width(&right), 40);

        // Degenerate widths never panic or overflow.
        for w in 0..6 {
            let (left, right) = status_bar(status, Some("⠋ 1 running"), w);
            assert!(width(&left) + width(&right) <= usize::from(w));
        }
    }

    #[test]
    fn status_and_version_never_overwrite_each_other() {
        let mut s = state(60, 20);
        s.status = "✗ Error: revit: connection refused (os error 61) — is Revit up?".into();
        let lines = text(&render(&s));
        assert!(lines[19].contains("✗ Error: revit: connection refused"));
        assert!(lines[19].trim_end().ends_with('…'), "{:?}", lines[19]);
        assert!(!lines[19].contains("ratatui"));
    }

    #[test]
    fn header_hints_give_way_to_the_title_on_narrow_screens() {
        let lines = text(&render(&state(120, 20)));
        assert!(lines[0].contains("x·stop") && lines[0].contains("Tab·focus"));

        let lines = text(&render(&state(30, 20)));
        assert!(lines[0].contains("makit"));
        assert!(lines[0].contains("?·help"));
        assert!(!lines[0].contains("Tab·focus"));

        let lines = text(&render(&state(12, 20)));
        assert!(lines[0].contains("makit"));
        assert!(!lines[0].contains("help"));
    }

    // ─── Help ───────────────────────────────────────────────────────────

    #[test]
    fn help_scrolls_on_a_short_terminal() {
        let mut s = state(80, 12);
        s.show_help = true;
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("Move selection")));
        assert!(!lines.iter().any(|l| l.contains("Groups —")));
        assert!(lines[11].contains("scroll"), "{:?}", lines[11]);

        s.help_scroll = layout::help_max_scroll(s.viewport);
        let lines = text(&render(&s));
        assert!(!lines.iter().any(|l| l.contains("Move selection")));
        assert!(lines.iter().any(|l| l.contains("Groups —")));
    }

    #[test]
    fn help_that_fits_says_how_to_close_it() {
        let mut s = state(120, 40);
        s.show_help = true;
        let lines = text(&render(&s));
        let bottom = row_of(&lines, "Esc or ? to close");
        assert!(
            lines[bottom].contains('╰'),
            "hint sits on the bottom border"
        );
        assert!(!lines.iter().any(|l| l.contains("scroll ·")));
    }

    // ─── Options ────────────────────────────────────────────────────────

    fn murb() -> NodeId {
        NodeId::Action("murb-simulate".into())
    }

    /// murb-simulate with `--epw` (required), `--cop-htg` (default 0.85) and
    /// `--verbose` (flag), expanded in the tree.
    fn with_options(w: u16, h: u16) -> AppState {
        let spec = |name: &str, kind: &str, required: bool, default: Option<&str>| OptionSpec {
            name: name.into(),
            description: format!("About {name}"),
            kind: kind.into(),
            required,
            default: default.map(str::to_string),
        };
        let opt = |name: &str, label: &str| {
            TreeNode::new(format!("action:murb-simulate:opt:{name}"), label)
                .description(format!("About {name}"))
        };
        let mut s = AppState::new(InitialData {
            tree: vec![TreeNode::new("actions", "Actions").child(
                TreeNode::new("action:murb-simulate", "▸ murb-simulate")
                    .child(opt("epw", "--epw: Path to EPW weather file *"))
                    .child(opt("cop-htg", "--cop-htg: Heating COP [0.85]"))
                    .child(opt("verbose", "--verbose: Chatty")),
            )],
            viewport: Rect::new(0, 0, w, h),
            ..InitialData::default()
        });
        s.catalog.insert(
            murb(),
            vec![
                spec("epw", "string", true, None),
                spec("cop-htg", "float", false, Some("0.85")),
                spec("verbose", "bool", false, None),
            ],
        );
        s.tree.select(1);
        s.tree.expand();
        s
    }

    #[test]
    fn option_rows_show_values_the_user_set() {
        let mut s = with_options(120, 40);
        let lines = text(&render(&s));
        // Untouched rows keep their registered description.
        assert!(lines
            .iter()
            .any(|l| l.contains("--epw: Path to EPW weather file *")));
        assert!(lines.iter().any(|l| l.contains("--cop-htg: Heating COP")));

        s.values
            .entry(murb())
            .or_default()
            .insert("epw".into(), "/w/van.epw".into());
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("--epw = /w/van.epw")));
        assert!(lines.iter().any(|l| l.contains("--cop-htg: Heating COP")));
    }

    #[test]
    fn a_focused_option_names_its_task_and_shows_its_value() {
        let mut s = with_options(120, 40);
        s.tree.select(2); // --epw
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("Focus: murb-simulate --epw")));
        assert!(lines.iter().any(|l| l.contains("About epw")));
        let detail = row_of(&lines, "string · required");
        assert!(lines[detail].contains("not set"), "{:?}", lines[detail]);
        assert!(lines.iter().any(|l| l.contains("⏎·edit")));

        s.values
            .entry(murb())
            .or_default()
            .insert("epw".into(), "/w/van.epw".into());
        let lines = text(&render(&s));
        let detail = row_of(&lines, "string · required");
        assert!(lines[detail].contains("/w/van.epw"), "{:?}", lines[detail]);

        s.tree.select(3); // --cop-htg
        let lines = text(&render(&s));
        let detail = row_of(&lines, "float · optional");
        assert!(
            lines[detail].contains("0.85 (default)"),
            "{:?}",
            lines[detail]
        );

        s.tree.select(4); // --verbose
        let lines = text(&render(&s));
        assert!(lines.iter().any(|l| l.contains("⏎·toggle")));
    }

    // ─── Editor form ────────────────────────────────────────────────────

    fn render_with_cursor(state: &AppState) -> (Vec<String>, Option<Position>) {
        (text(&render(state)), cursor_position(state, state.viewport))
    }

    #[test]
    fn the_terminal_cursor_follows_the_editor() {
        let mut s = with_options(120, 40);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        s.editor = Some(Editor::new(murb(), "epw", "abc", false));
        terminal.draw(|f| view(f, &s)).unwrap();
        let want = cursor_position(&s, s.viewport).unwrap();
        terminal.backend_mut().assert_cursor_position(want);
    }

    #[test]
    fn nothing_shows_a_cursor_until_the_editor_opens() {
        let s = with_options(120, 40);
        let (_, cursor) = render_with_cursor(&s);
        assert_eq!(cursor, None);
    }

    #[test]
    fn editor_form_shows_the_field_with_the_cursor_after_the_text() {
        let mut s = with_options(120, 40);
        s.editor = Some(Editor::new(murb(), "epw", "/w/van.epw", true));
        let (lines, cursor) = render_with_cursor(&s);
        assert!(lines.iter().any(|l| l.contains("murb-simulate --epw")));
        assert!(lines.iter().any(|l| l.contains("About epw")));
        assert!(lines.iter().any(|l| l.contains("string · required")));
        assert!(lines.iter().any(|l| l.contains("⏎ run")));
        assert!(lines.iter().any(|l| l.contains("Esc cancel")));

        let row = row_of(&lines, "› /w/van.epw");
        let col = lines[row].chars().position(|c| c == '›').unwrap();
        let cursor = cursor.expect("cursor is shown");
        assert_eq!(usize::from(cursor.y), row);
        assert_eq!(usize::from(cursor.x), col + 2 + "/w/van.epw".len());
    }

    #[test]
    fn editor_form_says_save_when_it_will_not_run_anything() {
        let mut s = with_options(120, 40);
        s.editor = Some(Editor::new(murb(), "cop-htg", "0.85", false));
        let (lines, _) = render_with_cursor(&s);
        assert!(lines.iter().any(|l| l.contains("⏎ save")));
        assert!(lines.iter().any(|l| l.contains("default 0.85")));
    }

    #[test]
    fn editor_keeps_the_cursor_in_view_for_long_values() {
        let mut s = with_options(120, 40);
        let long: String = ('a'..='z').cycle().take(200).collect();
        s.editor = Some(Editor::new(murb(), "epw", &long, false));
        let popup = layout::editor_rect(s.viewport);
        let (lines, cursor) = render_with_cursor(&s);
        let cursor = cursor.expect("cursor is shown");
        assert!(popup.contains(cursor), "{cursor:?} outside {popup:?}");
        assert!(cursor.x < popup.right() - 1, "cursor sits on the border");
        // The end of the value is what is visible.
        let tail: String = long.chars().skip(190).collect();
        assert!(lines[usize::from(cursor.y)].contains(&tail));

        // Moving to the start scrolls back.
        s.editor.as_mut().unwrap().home();
        let (lines, cursor) = render_with_cursor(&s);
        let cursor = cursor.unwrap();
        assert!(lines[usize::from(cursor.y)].contains("› abcdefgh"));
    }

    #[test]
    fn editor_fits_tiny_terminals() {
        for (w, h) in [(1, 1), (20, 5), (40, 7)] {
            let mut s = with_options(w, h);
            s.editor = Some(Editor::new(murb(), "epw", "/some/long/path/file.epw", true));
            let (_, cursor) = render_with_cursor(&s);
            if let Some(cursor) = cursor {
                assert!(cursor.x < w && cursor.y < h);
            }
        }
    }

    // ─── IFC errors ─────────────────────────────────────────────────────

    #[test]
    fn a_failed_ifc_load_is_shown_as_failed() {
        let mut s = state(120, 40);
        s.tree.select(1); // source:ifc
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        s.ifc.error = Some("missing field `walls` at line 3".into());
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("✗ Building-Architecture.ifc")));
        assert!(lines
            .iter()
            .any(|l| l.contains("missing field `walls` at line 3")));
        assert!(!lines.iter().any(|l| l.contains("✓")));
        assert!(!lines.iter().any(|l| l.contains("Not extracted")));
    }

    #[test]
    fn an_ifc_without_extraction_says_what_is_missing() {
        let mut s = state(120, 40);
        s.tree.select(1);
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        let lines = text(&render(&s));
        // The detail line does not claim success for a file with no results.
        assert!(lines
            .iter()
            .any(|l| l.contains("◆ Building-Architecture.ifc")));
        assert!(!lines
            .iter()
            .any(|l| l.contains("✓ Building-Architecture.ifc") && !l.contains("│✓")));
        assert!(lines
            .iter()
            .any(|l| l.contains("Not extracted yet: no Building-Architecture.json")));
    }

    #[test]
    fn a_missing_extraction_shows_its_reason() {
        let mut s = state(120, 40);
        s.tree.select(1);
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        s.ifc.note = Some("no Python with ifcopenshell: set MAKIT_PYTHON".into());
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("no Python with ifcopenshell")));
        assert!(!lines.iter().any(|l| l.contains("Not extracted yet")));
    }

    fn sample_model() -> building_model::BuildingModel {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/building-model.json"
        );
        building_model::load_model(Path::new(path)).expect("example model")
    }

    #[test]
    fn the_detail_shows_the_envelope_line_and_a_note() {
        let mut s = state(120, 40);
        s.tree.select(1);
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        let model = sample_model();
        s.ifc.analysis = Some(building_model::analyze_wwr(&model));
        s.ifc.envelope = Some(building_model::envelope(&model, 0.0));
        s.ifc.note = Some("using stale building-model.json: no python".into());
        let lines = text(&render(&s));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("GFA 26 m²") && l.contains("roof 54 m²")),
            "{lines:#?}"
        );
        assert!(lines
            .iter()
            .any(|l| l.contains("using stale building-model.json")));
    }

    // ─── Honest previews ────────────────────────────────────────────────

    #[test]
    fn static_drawings_are_labelled_illustrations() {
        let mut s = state(120, 40);
        s.tree.select(2); // source:revit
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("Preview · isometric · illustration")));
    }

    #[test]
    fn a_loaded_analysis_is_drawn_as_the_wall_rose() {
        let mut s = state(120, 40);
        s.tree.select(1); // source:ifc
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/IFC/building-model.json"
        );
        s.ifc.loaded = Some(s.ifc.files[0].clone());
        s.ifc.analysis = Some(building_model::analyze_wwr(
            &building_model::load_model(Path::new(path)).unwrap(),
        ));
        let lines = text(&render(&s));
        assert!(lines
            .iter()
            .any(|l| l.contains("Preview · wall area by orientation")));
        assert!(has_braille(&lines));
    }
}
