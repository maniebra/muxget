use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Cell, Gauge, Padding, Paragraph, Row, Sparkline, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::downloads::Filter;
use crate::models::download::{Download, Status};
use crate::utils::parse::human;
use crate::views::theme::Theme;

/// Widths at which panels earn their space. Below each, the panel is dropped
/// rather than squeezed into something unreadable.
const SIDEBAR_MIN: u16 = 90;
const DETAILS_MIN: u16 = 64;
const SPARK_MIN_HEIGHT: u16 = 20;

pub fn draw(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let area = f.area();

    // Paint the window background first; every panel draws on top of it.
    f.render_widget(Block::default().style(Style::default().bg(t.bg).fg(t.fg)), area);

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .areas(area);

    // Responsive: sidebars appear only when the terminal is wide enough.
    let cols = match (area.width >= SIDEBAR_MIN, area.width >= DETAILS_MIN) {
        (true, _) => vec![Constraint::Length(20), Constraint::Min(30), Constraint::Length(32)],
        (false, true) => vec![Constraint::Min(30), Constraint::Length(30)],
        (false, false) => vec![Constraint::Min(0)],
    };
    let panes = Layout::horizontal(cols).split(body);

    let (sidebar, main, details) = match panes.len() {
        3 => (Some(panes[0]), panes[1], Some(panes[2])),
        2 => (None, panes[0], Some(panes[1])),
        _ => (None, panes[0], None),
    };

    draw_header(f, app, header);
    if let Some(area) = sidebar {
        draw_sidebar(f, app, area);
    }

    // Throughput graph only when there is vertical room to spare.
    if area.height >= SPARK_MIN_HEIGHT {
        let [list, spark] = Layout::vertical([Constraint::Min(3), Constraint::Length(7)]).areas(main);
        draw_table(f, app, list);
        draw_sparkline(f, app, spark);
    } else {
        draw_table(f, app, main);
    }

    if let Some(area) = details {
        draw_details(f, app, area);
    }
    draw_footer(f, app, footer);
    // Last, so popovers sit on top of everything.
    crate::views::dialog::draw(f, app);
    crate::views::options::draw(f, app);
}

/// Bordered panel on the sidebar/alt background.
fn panel(t: &Theme, title: &str, bg: ratatui::style::Color) -> Block<'static> {
    Block::bordered()
        .border_style(Style::default().fg(t.muted).bg(bg))
        .style(Style::default().bg(bg).fg(t.fg))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(t.accent).bg(bg).add_modifier(Modifier::BOLD),
        ))
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let count = |f: Filter| app.downloads.iter().filter(|d| f.matches(&d.status)).count();
    let chip = |text: String, fg| Span::styled(text, Style::default().fg(fg).bg(t.panel));
    let sep = chip(" │ ".into(), t.muted);

    let mut spans = vec![
        Span::styled(
            " muxget ",
            Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        Span::styled(
            format!(" {} ", app.queue().name),
            Style::default().fg(t.fg).bg(t.selected).add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
        if app.queue().paused {
            Span::styled(
                " paused ",
                Style::default().fg(t.bg).bg(t.err).add_modifier(Modifier::BOLD),
            )
        } else {
            chip(
                format!("{}/{} running", app.active_in(app.queue().id), app.queue().max_active),
                t.accent,
            )
        },
        sep.clone(),
        chip(format!("{} queued", app.queued_in(app.queue().id)), t.muted),
        sep.clone(),
        chip(format!("{} done", count(Filter::Done)), t.ok),
        sep.clone(),
        chip(format!("{} failed", count(Filter::Failed)), t.err),
        sep.clone(),
        Span::styled(
            format!("{}/s", human(app.speed())),
            Style::default().fg(t.ok).bg(t.panel).add_modifier(Modifier::BOLD),
        ),
    ];
    // Path is the first thing to drop on a narrow terminal.
    if area.width > 70 {
        spans.push(sep);
        spans.push(chip(app.dir.display().to_string(), t.muted));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::bordered()
                .border_style(Style::default().fg(t.muted).bg(t.panel))
                .style(Style::default().bg(t.panel))
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let [queues, filters] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let lines: Vec<Line> = app
        .queues
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let on = i == app.current;
            let style = if on {
                Style::default().fg(t.accent).bg(t.selected).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg).bg(t.panel)
            };
            Line::styled(
                format!(
                    "{} {:<9}{}",
                    if on { "▸" } else { " " },
                    truncate(&q.name, 9),
                    match (q.paused, q.schedule) {
                        // A scheduled queue shows its window; it explains the pause.
                        (_, Some(_)) => q.window(),
                        (true, None) => "paused".to_string(),
                        (false, None) => format!("{}/{}", app.active_in(q.id), q.max_active),
                    }
                ),
                style,
            )
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines)
            .block(panel(t, "queues", t.panel).padding(Padding::horizontal(1))),
        queues,
    );

    let rows: Vec<Line> = Filter::ALL
        .iter()
        .map(|filter| {
            let n = app.downloads.iter().filter(|d| filter.matches(&d.status)).count();
            let on = *filter == app.filter;
            let style = if on {
                Style::default().fg(t.accent).bg(t.selected).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg).bg(t.panel)
            };
            Line::styled(format!("{} {:<8}{n:>3}", if on { "▸" } else { " " }, filter.label()), style)
        })
        .collect();
    f.render_widget(
        Paragraph::new(rows).block(panel(t, "filter", t.panel).padding(Padding::horizontal(1))),
        filters,
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

fn draw_table(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let visible = app.visible();
    let title = format!(
        "{} — {} ({})",
        app.queue().name,
        app.filter.label(),
        visible.len()
    );

    if visible.is_empty() {
        f.render_widget(
            Paragraph::new("\n  nothing here — press `a` to add a url")
                .style(Style::default().fg(t.muted).bg(t.bg))
                .block(panel(t, &title, t.bg)),
            area,
        );
        return;
    }

    // Drop columns as the pane narrows, widest-luxury first.
    let wide = area.width >= 74;
    let mid = area.width >= 52;

    let rows = visible.iter().enumerate().map(|(row, &i)| {
        let d = &app.downloads[i];
        let (icon, state, color) = match &d.status {
            Status::Queued => ("·", "queued".into(), t.muted),
            Status::Running => ("▶", d.progress.eta.clone(), t.accent),
            Status::Paused => ("⏸", "paused".into(), t.err),
            Status::Done => ("✓", "done".into(), t.ok),
            Status::Cancelled => ("■", "cancelled".into(), t.muted),
            Status::Failed(e) => ("✗", e.clone(), t.err),
        };
        // Zebra striping so long lists stay scannable.
        let bg = if row % 2 == 0 { t.bg } else { t.panel };
        let mut cells = vec![
            Cell::from(icon).style(Style::default().fg(color)),
            Cell::from(name_of(d)),
        ];
        cells.push(Cell::from(bar(d.progress.percent, if wide { 14 } else { 8 })).style(Style::default().fg(color)));
        cells.push(Cell::from(format!("{:>5.1}%", d.progress.percent)));
        if mid {
            cells.push(Cell::from(d.progress.speed.clone()).style(Style::default().fg(t.muted)));
            cells.push(Cell::from(state).style(Style::default().fg(color)));
        }
        Row::new(cells).style(Style::default().bg(bg).fg(t.fg))
    });

    let mut widths = vec![Constraint::Length(1), Constraint::Min(12)];
    let mut header = vec!["", "name"];
    widths.push(Constraint::Length(if wide { 14 } else { 8 }));
    widths.push(Constraint::Length(6));
    header.extend(["progress", ""]);
    if mid {
        widths.extend([Constraint::Length(10), Constraint::Length(12)]);
        header.extend(["speed", "eta"]);
    }

    let table = Table::new(rows, widths)
        .header(Row::new(header).style(
            Style::default().fg(t.muted).bg(t.bg).add_modifier(Modifier::BOLD),
        ))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(t.selected).fg(t.fg).add_modifier(Modifier::BOLD))
        .block(panel(t, &title, t.bg));

    // Fresh state each frame: ratatui scrolls to keep the selection visible.
    let at = visible.iter().position(|i| *i == app.selected);
    let mut state = TableState::new().with_selected(at);
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_sparkline(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let width = area.width.saturating_sub(2) as usize;
    let start = app.history.len().saturating_sub(width);
    f.render_widget(
        Sparkline::default()
            .data(&app.history[start..])
            .style(Style::default().fg(t.accent).bg(t.panel))
            .block(panel(t, &format!("throughput  {}/s", human(app.speed())), t.panel)),
        area,
    );
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(d) = app
        .downloads
        .get(app.selected)
        .filter(|_| app.visible().contains(&app.selected))
    else {
        f.render_widget(panel(t, "details", t.panel), area);
        return;
    };

    let [top, gauge] = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).areas(area);
    let field = |k: &str, v: String, c| {
        Line::from(vec![
            Span::styled(format!("{k:<8}"), Style::default().fg(t.muted).bg(t.panel)),
            Span::styled(v, Style::default().fg(c).bg(t.panel)),
        ])
    };
    let (state, color) = match &d.status {
        Status::Queued => ("queued".to_string(), t.muted),
        Status::Running => ("running".to_string(), t.accent),
        Status::Paused => ("paused".to_string(), t.err),
        Status::Done => ("done".to_string(), t.ok),
        Status::Cancelled => ("cancelled".to_string(), t.muted),
        Status::Failed(e) => (format!("failed ({e})"), t.err),
    };

    f.render_widget(
        Paragraph::new(vec![
            field("name", name_of(d), color),
            field("status", state, color),
            field("speed", d.progress.speed.clone(), t.accent),
            field("eta", d.progress.eta.clone(), t.accent),
            Line::default(),
            Line::styled("url", Style::default().fg(t.muted).bg(t.panel)),
            Line::styled(d.url.clone(), Style::default().fg(t.fg).bg(t.panel)),
        ])
        .wrap(Wrap { trim: true })
        .block(panel(t, "details", t.panel).padding(Padding::horizontal(1))),
        top,
    );

    f.render_widget(
        Gauge::default()
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.muted).bg(t.panel))
                    .style(Style::default().bg(t.panel)),
            )
            .gauge_style(Style::default().fg(color).bg(t.selected))
            .label(format!("{:.1}%", d.progress.percent))
            .ratio((d.progress.percent as f64 / 100.0).clamp(0.0, 1.0)),
        gauge,
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let keys: &[(&str, &str)] = if area.width >= 100 {
        &[("a", "add"), ("e", "edit"), ("d", "del"), ("p", "pause"), ("x", "stop"), ("g…", "queue"), ("s…", "settings"), ("[ ]", "switch"), ("Tab", "filter"), ("q", "quit")]
    } else if area.width >= 74 {
        &[("a", "add"), ("d", "del"), ("g…", "queue"), ("s…", "settings"), ("Tab", "filter"), ("q", "quit")]
    } else {
        &[("a", "add"), ("g…", "queue"), ("s…", "settings"), ("q", "quit")]
    };

    let mut spans = Vec::new();
    for (key, label) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}  "), Style::default().fg(t.muted).bg(t.panel)));
    }
    spans.push(Span::styled(app.message.clone(), Style::default().fg(t.ok).bg(t.panel)));

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .block(panel(t, &format!("theme: {}", t.name), t.panel).padding(Padding::horizontal(1))),
        area,
    );
}

/// Filename from the url, falling back to the url itself.
fn name_of(d: &Download) -> String {
    d.url
        .split(['?', '#'])
        .next()
        .unwrap_or(&d.url)
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(&d.url)
        .to_string()
}

fn bar(percent: f32, width: usize) -> String {
    let filled = ((percent / 100.0 * width as f32).round() as usize).min(width);
    "█".repeat(filled) + &"░".repeat(width - filled)
}
