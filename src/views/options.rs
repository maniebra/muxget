use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Padding, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::options::{Options, Settings, GENERAL, RULE_ROWS, TABS};
use crate::models::log;
use crate::models::rule::FIELDS as RULE_FIELDS;
use crate::models::option::Kind;
use crate::utils::{args, config_dir, edit};

pub fn draw(f: &mut Frame, app: &App) {
    let Some(panel) = &app.settings else { return };
    let t = &app.theme;
    let area = centered(f.area(), 78, 24);
    f.render_widget(Clear, area);

    let [tabs, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .areas(area);

    draw_tabs(f, app, panel, tabs);
    match panel.tab {
        0 => draw_general(f, app, panel, body),
        1 => draw_form(f, app, panel, &panel.options, body),
        2 => draw_form(f, app, panel, &panel.crawl, body),
        3 => draw_categories(f, app, panel, body),
        4 => draw_channels(f, app, panel, body),
        _ => draw_log(f, app, panel, body),
    }

    let path = match panel.tab {
        1 => args::path(panel.options.backend),
        2 => args::path("crawl"),
        3 => config_dir().join("rules"),
        4 => crate::models::channel::path(),
        // The log is in memory only; nothing to point at on disk.
        _ => std::path::PathBuf::from("in memory · x clears · G newest"),
    };
    f.render_widget(
        Paragraph::new(hints(panel, t))
            .style(Style::default().bg(t.panel))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.muted).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::horizontal(1))
                    .title_bottom(Span::styled(
                        format!(" {} ", path.display()),
                        Style::default().fg(t.muted).bg(t.panel),
                    )),
            ),
        footer,
    );
}

fn draw_tabs(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    let mut spans = Vec::new();
    for (i, name) in TABS.iter().enumerate() {
        let on = i == panel.tab;
        // The backends tab names the one it is showing.
        let name = match i == 1 {
            true => format!("{name}: {}", panel.options.backend),
            false => name.to_string(),
        };
        spans.push(Span::styled(
            format!(" {name} "),
            if on {
                Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.muted).bg(t.panel)
            },
        ));
        spans.push(Span::styled(" ", Style::default().bg(t.panel)));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::bordered()
                .border_style(Style::default().fg(t.accent).bg(t.panel))
                .style(Style::default().bg(t.panel))
                .padding(Padding::horizontal(1))
                .title(Span::styled(
                    " settings ",
                    Style::default().fg(t.accent).bg(t.panel).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_general(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    let values = [
        app.theme.name.to_string(),
        app.dir.display().to_string(),
        if app.nerd { "on".into() } else { "off".into() },
        if app.confirm_playlist { "on".into() } else { "off".into() },
    ];
    let rows = GENERAL.iter().zip(values).enumerate().map(|(i, (label, value))| {
        Row::new(vec![
            Cell::from(*label),
            Cell::from(value).style(Style::default().fg(t.ok)),
        ])
        .style(Style::default().bg(if i % 2 == 0 { t.panel } else { t.bg }).fg(t.fg))
    });
    render_table(f, app, panel.cursor, rows, [Constraint::Length(24), Constraint::Min(20)], area);
}

fn draw_form(f: &mut Frame, app: &App, panel: &Settings, opts: &Options, area: Rect) {
    let t = &app.theme;
    let rows = opts.specs().iter().enumerate().map(|(i, spec)| {
        let editing = opts.editing.as_ref().filter(|_| i == panel.cursor);
        let (mark, value, color) = match (&spec.kind, opts.value(spec.flag), editing) {
            (_, _, Some(buf)) => ("▸".into(), edit::caret(buf, opts.caret), t.accent),
            (Kind::Flag, _, _) if opts.is_set(spec.flag) => ("[x]".into(), String::new(), t.ok),
            (Kind::Flag, _, _) => ("[ ]".into(), String::new(), t.muted),
            (Kind::Choice(presets), Some(v), _) => (
                "▸".into(),
                // A hand-written selector is shown as it is, not guessed at.
                match opts.preset(spec.flag, presets) {
                    Some(preset) => preset.label.to_string(),
                    None => format!("custom: {v}"),
                },
                t.ok,
            ),
            (Kind::Value, Some(v), _) => ("▸".into(), v.to_string(), t.ok),
            (Kind::Value | Kind::Choice(_), None, _) => {
                (" ".to_string(), format!("— {}", spec.hint), t.muted)
            }
        };
        Row::new(vec![
            Cell::from(mark).style(Style::default().fg(color)),
            Cell::from(spec.label),
            Cell::from(spec.flag).style(Style::default().fg(t.muted)),
            Cell::from(value).style(Style::default().fg(color)),
        ])
        .style(Style::default().bg(if i % 2 == 0 { t.panel } else { t.bg }).fg(t.fg))
    });
    render_table(
        f,
        app,
        panel.cursor,
        rows,
        [
            Constraint::Length(3),
            Constraint::Min(20),
            Constraint::Length(26),
            Constraint::Length(16),
        ],
        area,
    );
}

/// Rules are read from a file and shown as they will be applied; editing them
/// is what a text editor is for.
fn draw_categories(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    if panel.rules.is_empty() {
        f.render_widget(
            Paragraph::new("\n  no rules yet — press `n` to add one")
                .style(Style::default().bg(t.panel).fg(t.muted))
                .block(
                    Block::bordered()
                        .border_style(Style::default().fg(t.muted).bg(t.panel))
                        .style(Style::default().bg(t.panel))
                        .padding(Padding::horizontal(1)),
                ),
            area,
        );
        return;
    }
    // One header row per rule, then a row per field of it — a single cursor
    // over the lot, so there is no mode to be in.
    let mut rows = Vec::new();
    for (at, rule) in panel.rules.iter().enumerate() {
        let (what, to) = rule.summary();
        let what = match what.is_empty() {
            true => "every url".to_string(),
            false => what,
        };
        rows.push(
            Row::new(vec![
                Cell::from(format!("rule {}", at + 1)),
                Cell::from(what).style(Style::default().fg(t.fg)),
                Cell::from(to).style(Style::default().fg(t.ok)),
            ])
            .style(Style::default().bg(t.bg).add_modifier(Modifier::BOLD)),
        );
        for (i, name) in RULE_FIELDS.iter().enumerate() {
            let row = at * RULE_ROWS + 1 + i;
            let editing = panel.editing_rule.as_ref().filter(|(r, _)| *r == row);
            let (value, color) = match (editing, rule.get(i)) {
                (Some((_, buf)), _) => (edit::caret(buf, panel.caret), t.accent),
                (None, v) if v.is_empty() => (format!("— {}", RULE_HINTS[i]), t.muted),
                (None, v) => (v, t.ok),
            };
            rows.push(
                Row::new(vec![
                    Cell::from(""),
                    Cell::from(format!("  {name}")),
                    Cell::from(value).style(Style::default().fg(color)),
                ])
                .style(Style::default().bg(t.panel).fg(t.fg)),
            );
        }
    }
    render_table(
        f,
        app,
        panel.cursor,
        rows.into_iter(),
        [Constraint::Length(8), Constraint::Length(22), Constraint::Min(20)],
        area,
    );
}

/// Channels to keep up with: one row each, with the day it was last synced.
/// Syncing lists everything uploaded since that day and moves it to today.
fn draw_channels(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    if panel.channels.is_empty() && panel.editing_channel.is_none() {
        f.render_widget(
            Paragraph::new("\n  no channels yet — press `n` to add one")
                .style(Style::default().bg(t.panel).fg(t.muted))
                .block(
                    Block::bordered()
                        .border_style(Style::default().fg(t.muted).bg(t.panel))
                        .style(Style::default().bg(t.panel))
                        .padding(Padding::horizontal(1)),
                ),
            area,
        );
        return;
    }
    let rows = panel.channels.iter().enumerate().map(|(i, c)| {
        let editing = panel.editing_channel.as_ref().filter(|(row, _, _)| *row == i);
        let (url, date) = match editing {
            Some((_, true, buf)) => (c.url.clone(), edit::caret(buf, panel.caret)),
            Some((_, false, buf)) => (edit::caret(buf, panel.caret), shown_date(&c.last_sync)),
            None => (c.url.clone(), shown_date(&c.last_sync)),
        };
        Row::new(vec![
            Cell::from(url).style(Style::default().fg(t.fg)),
            Cell::from(date).style(Style::default().fg(match c.last_sync.is_empty() {
                true => t.muted,
                false => t.ok,
            })),
        ])
        .style(Style::default().bg(if i % 2 == 0 { t.panel } else { t.bg }))
    });
    render_table(
        f,
        app,
        panel.cursor,
        rows,
        [Constraint::Min(20), Constraint::Length(14)],
        area,
    );
}

/// A stored `20240131` as `2024-01-31`; never synced reads as what it means.
fn shown_date(date: &str) -> String {
    match date.len() == 8 {
        true => format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
        false => "never synced".to_string(),
    }
}

/// The log, oldest first, scrolled to wherever the cursor is. Read only —
/// what happened, happened.
fn draw_log(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    let entries = log::entries();
    if entries.is_empty() {
        f.render_widget(
            Paragraph::new("\n  nothing logged yet")
                .style(Style::default().bg(t.panel).fg(t.muted))
                .block(
                    Block::bordered()
                        .border_style(Style::default().fg(t.muted).bg(t.panel))
                        .style(Style::default().bg(t.panel))
                        .padding(Padding::horizontal(1)),
                ),
            area,
        );
        return;
    }
    let rows = entries.iter().skip(panel.cursor).map(|e| {
        let color = match e.level {
            log::Level::Info => t.muted,
            log::Level::Warn => t.accent,
            log::Level::Error => t.err,
        };
        Row::new(vec![
            Cell::from(e.at.clone()).style(Style::default().fg(t.muted)),
            Cell::from(match e.level {
                log::Level::Info => " ",
                log::Level::Warn => "!",
                log::Level::Error => "✗",
            })
            .style(Style::default().fg(color)),
            Cell::from(e.text.clone()).style(Style::default().fg(color)),
        ])
        .style(Style::default().bg(t.panel).fg(t.fg))
    });
    // The cursor is the scroll here: the table shows from it down, so the
    // highlighted row is always the top one.
    render_table(
        f,
        app,
        0,
        rows,
        [Constraint::Length(8), Constraint::Length(1), Constraint::Min(20)],
        area,
    );
}

/// What each rule field is for, shown while it is empty.
const RULE_HINTS: [&str; 7] = [
    "e.g. mp4,mkv",
    "e.g. youtube.com",
    "e.g. youtube.com/@* — each `*` becomes $1, $2 …",
    "e.g. 500M — routes once the size is known",
    "queue to send it to — `$1` allowed",
    "directory to save it in — `$1` allowed",
    "aria2c | yt-dlp | wget",
];

fn render_table<'a>(
    f: &mut Frame,
    app: &App,
    cursor: usize,
    rows: impl Iterator<Item = Row<'a>>,
    widths: impl IntoIterator<Item = Constraint>,
    area: Rect,
) {
    let t = &app.theme;
    let mut state = TableState::new().with_selected(Some(cursor));
    f.render_stateful_widget(
        Table::new(rows, widths)
            .column_spacing(1)
            .row_highlight_style(Style::default().bg(t.selected).add_modifier(Modifier::BOLD))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.muted).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::horizontal(1)),
            ),
        area,
        &mut state,
    );
}

fn hints<'a>(panel: &Settings, t: &crate::views::theme::Theme) -> Line<'a> {
    let editing = panel.options.editing.is_some()
        || panel.crawl.editing.is_some()
        || panel.editing_rule.is_some()
        || panel.editing_channel.is_some();
    let keys: &[(&str, &str)] = match (panel.tab, editing) {
        (1..=4, true) => &[("Enter", "set"), ("Esc", "cancel"), ("empty", "unset")],
        (0, _) => &[
            ("Tab", "next tab"),
            ("j/k", "move"),
            ("Enter", "change"),
            ("Esc", "close"),
        ],
        (1, _) => &[
            ("Tab", "next tab"),
            ("j/k", "move"),
            ("Enter", "toggle / edit"),
            ("b", "backend"),
            ("x", "unset"),
            ("Esc", "save & close"),
        ],
        (2, _) => &[
            ("Tab", "next tab"),
            ("j/k", "move"),
            ("Enter", "toggle / edit"),
            ("x", "unset"),
            ("Esc", "save & close"),
        ],
        (4, _) => &[
            ("Tab", "next tab"),
            ("Enter", "url"),
            ("d", "last sync"),
            ("n", "new"),
            ("x", "delete"),
            ("s/S", "sync one / all"),
        ],
        (5, _) => &[
            ("Tab", "next tab"),
            ("j/k", "scroll"),
            ("g/G", "oldest / newest"),
            ("x", "clear the log"),
            ("Esc", "close"),
        ],
        (3, _) => &[
            ("Tab", "next tab"),
            ("j/k", "move"),
            ("Enter", "edit"),
            ("n", "new rule"),
            ("x", "clear field / delete rule"),
            ("Esc", "save & close"),
        ],
        _ => &[("Tab", "next tab"), ("Esc", "close")],
    };
    let mut spans = Vec::new();
    for (key, label) in keys {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::default().fg(t.muted).bg(t.panel),
        ));
    }
    Line::from(spans)
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let [row] = Layout::vertical([Constraint::Length(h)]).flex(Flex::Center).areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(w)]).flex(Flex::Center).areas(row);
    cell
}
