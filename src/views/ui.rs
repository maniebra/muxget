use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Cell, Gauge, Padding, Paragraph, Row, Sparkline, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::downloads::Filter;
use crate::models::download::{Download, Progress, Status};
use crate::utils::parse::{bytes, human};
use crate::views::theme::Theme;

/// Widths at which panels earn their space. Below each, the panel is dropped
/// rather than squeezed into something unreadable.
const SIDEBAR_MIN: u16 = 90;
const DETAILS_MIN: u16 = 64;
const SPARK_MIN_HEIGHT: u16 = 20;

/// Where each panel sits. The mouse handler needs the same answer the drawing
/// code uses, so both go through `layout`.
pub struct Panes {
    pub queues: Option<Rect>,
    pub filters: Option<Rect>,
    pub list: Rect,
    pub spark: Option<Rect>,
    pub details: Option<Rect>,
    pub header: Rect,
    pub footer: Rect,
}

pub fn layout(area: Rect) -> Panes {
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
    let (queues, filters) = match sidebar {
        Some(area) => {
            let [queues, filters] =
                Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);
            (Some(queues), Some(filters))
        }
        None => (None, None),
    };

    // Throughput graph only when there is vertical room to spare.
    let (list, spark) = if area.height >= SPARK_MIN_HEIGHT {
        let [list, spark] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(7)]).areas(main);
        (list, Some(spark))
    } else {
        (main, None)
    };

    Panes { queues, filters, list, spark, details, header, footer }
}

pub fn draw(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let area = f.area();

    // Paint the window background first; every panel draws on top of it.
    f.render_widget(Block::default().style(Style::default().bg(t.bg).fg(t.fg)), area);

    let p = layout(area);
    draw_header(f, app, p.header);
    if let (Some(queues), Some(filters)) = (p.queues, p.filters) {
        draw_sidebar(f, app, queues, filters);
    }
    draw_table(f, app, p.list);
    if let Some(area) = p.spark {
        draw_sparkline(f, app, area);
    }
    if let Some(area) = p.details {
        draw_details(f, app, area);
    }
    draw_footer(f, app, p.footer);
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
            format!("↓{}/s", human(app.speed())),
            Style::default().fg(t.ok).bg(t.panel).add_modifier(Modifier::BOLD),
        ),
    ];
    if app.upload() > 0.0 {
        spans.push(sep.clone());
        spans.push(chip(format!("↑{}/s", human(app.upload())), t.accent));
    }
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

fn draw_sidebar(f: &mut Frame, app: &App, queues: Rect, filters: Rect) {
    let t = &app.theme;

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
                    match (q.paused, q.scheduled()) {
                        // A schedule explains the pause it causes.
                        (_, true) => truncate(&q.window(), 11),
                        (true, false) => "paused".to_string(),
                        (false, false) => format!("{}/{}", app.active_in(q.id), q.max_active),
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
        let (state, color) = match &d.status {
            Status::Queued => ("queued".into(), t.muted),
            // A torrent at 100% is still running: it is seeding, not stalled.
            Status::Running if d.progress.is_torrent() && d.progress.percent >= 100.0 => {
                ("seeding".into(), t.ok)
            }
            Status::Running => (d.progress.eta.clone(), t.accent),
            Status::Paused => ("paused".into(), t.err),
            Status::Done => ("done".into(), t.ok),
            Status::Cancelled => ("cancelled".into(), t.muted),
            Status::Failed(e) => (e.clone(), t.err),
        };
        let icon = status_icon(&d.status, app.nerd);
        // Zebra striping so long lists stay scannable.
        let bg = if row % 2 == 0 { t.bg } else { t.panel };
        let mut cells = vec![
            Cell::from(icon).style(Style::default().fg(color)),
            Cell::from(name_of(d)),
        ];
        cells.push(Cell::from(bar(d.progress.percent, if wide { 14 } else { 8 })).style(Style::default().fg(color)));
        cells.push(Cell::from(format!("{:>5.1}%", d.progress.percent)));
        if mid {
            cells.push(Cell::from(rates(&d.progress)).style(Style::default().fg(t.muted)));
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
    // Borders, padding and the label column. A value that does not fit is cut:
    // a magnet link is thousands of characters and would fill the panel.
    let inner = area.width.saturating_sub(4) as usize;
    let room = inner.saturating_sub(9);
    let field = |k: &str, v: String, c| {
        Line::from(vec![
            Span::styled(format!("{k:<9}"), Style::default().fg(t.muted).bg(t.panel)),
            Span::styled(truncate(&v, room), Style::default().fg(c).bg(t.panel)),
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
            field(
                "status",
                format!("{} {}", status_icon(&d.status, app.nerd), state),
                color,
            ),
            if d.progress.total.is_empty() {
                Line::default()
            } else {
                field(
                    "size",
                    format!("{} / {}", d.progress.done, d.progress.total),
                    t.fg,
                )
            },
            field("speed", d.progress.speed.clone(), t.accent),
            field("eta", d.progress.eta.clone(), t.accent),
            if d.progress.is_torrent() {
                field("upload", swarm_upload(&d.progress), t.accent)
            } else {
                Line::default()
            },
            if d.progress.is_torrent() {
                field("peers", d.progress.peers.to_string(), t.fg)
            } else {
                Line::default()
            },
            if d.progress.is_torrent() {
                field(
                    "seeders",
                    d.progress.seeders.unwrap_or(0).to_string(),
                    t.ok,
                )
            } else {
                Line::default()
            },
            if d.progress.is_torrent() {
                field("leechers", d.progress.leechers().to_string(), t.muted)
            } else {
                Line::default()
            },
            if d.over.user.is_empty() {
                Line::default()
            } else {
                // The user name only; never the password.
                field("login", d.over.user.clone(), t.muted)
            },
            Line::styled("url", Style::default().fg(t.muted).bg(t.panel)),
            // Three wrapped lines of it, so trackers cannot push the fields
            // above off the panel.
            Line::styled(
                truncate(&d.url, inner * 3),
                Style::default().fg(t.fg).bg(t.panel),
            ),
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
        &[("a", "add"), ("e", "edit"), ("d", "del"), ("p", "pause"), ("x", "stop"), ("i…", "item"), ("g…", "queue"), ("s", "settings"), ("[ ]", "switch"), ("Tab", "filter"), ("q", "quit")]
    } else if area.width >= 74 {
        &[("a", "add"), ("d", "del"), ("i…", "item"), ("g…", "queue"), ("s", "settings"), ("Tab", "filter"), ("q", "quit")]
    } else {
        &[("a", "add"), ("g…", "queue"), ("s", "settings"), ("q", "quit")]
    };

    let mut spans = Vec::new();
    for (key, label) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {label}  "), Style::default().fg(t.muted).bg(t.panel)));
    }
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let room = (area.width as usize).saturating_sub(used + 4);
    spans.push(Span::styled(
        truncate(&app.message, room),
        Style::default().fg(t.ok).bg(t.panel),
    ));

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .block(panel(t, &format!("theme: {}", t.name), t.panel).padding(Padding::horizontal(1))),
        area,
    );
}

/// What to call this download: the file a backend wrote, the name the user
/// asked for, then the url's. A url ending in a route rather than a file
/// (`/watch`) keeps its query, or every video would be called "watch".
pub fn name_of(d: &Download) -> String {
    if let Some(name) = d.path.as_ref().and_then(|p| p.file_name()) {
        return name.to_string_lossy().into_owned();
    }
    if !d.over.name.is_empty() {
        return d.over.name.clone();
    }
    // A magnet carries its display name in `dn`; the info hash tells nobody
    // anything.
    if d.url.starts_with("magnet:") {
        if let Some(dn) = d.url.split(['?', '&']).find_map(|p| p.strip_prefix("dn=")) {
            return dn.replace('+', " ");
        }
    }
    let (path, query) = match d.url.split_once('?') {
        Some((path, query)) => (path, query.split('#').next().unwrap_or("")),
        None => (d.url.split('#').next().unwrap_or(&d.url), ""),
    };
    let last = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&d.url);
    if query.is_empty() || last.contains('.') {
        last.to_string()
    } else {
        format!("{last}?{query}")
    }
}

/// Status glyph. Nerd font codepoints only if the user opted in; the fallback
/// box is worse than the plain unicode it replaces.
pub fn status_icon(status: &Status, nerd: bool) -> &'static str {
    match status {
        Status::Queued => if nerd { "\u{f017}" } else { "·" },
        Status::Running => if nerd { "\u{f019}" } else { "▶" },
        Status::Paused => if nerd { "\u{f04c}" } else { "⏸" },
        Status::Done => if nerd { "\u{f00c}" } else { "✓" },
        Status::Cancelled => if nerd { "\u{f04d}" } else { "■" },
        Status::Failed(_) => if nerd { "\u{f00d}" } else { "✗" },
    }
}

/// `1.2MiB` alone, or `1.2MiB (30MiB total)` once anything has been sent.
fn swarm_upload(p: &Progress) -> String {
    match p.uploaded.is_empty() {
        true => p.upload.clone(),
        false => format!("{} ({} total)", p.upload, p.uploaded),
    }
}

/// `5.0MiB` alone, or `5.0MiB ↑1.0MiB` while a torrent is uploading.
fn rates(p: &Progress) -> String {
    match p.upload.is_empty() || bytes(&p.upload).unwrap_or(0.0) == 0.0 {
        true => p.speed.clone(),
        false => format!("{} ↑{}", p.speed, p.upload),
    }
}

fn bar(percent: f32, width: usize) -> String {
    let filled = ((percent / 100.0 * width as f32).round() as usize).min(width);
    "█".repeat(filled) + &"░".repeat(width - filled)
}
