use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Padding, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::options::{Settings, GENERAL, TABS};
use crate::models::option::Kind;
use crate::utils::{args, config_dir};

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
        1 => draw_backend(f, app, panel, body),
        _ => draw_categories(f, app, body),
    }

    let path = match panel.tab {
        1 => args::path(panel.options.backend),
        _ => config_dir().join("rules"),
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

fn draw_backend(f: &mut Frame, app: &App, panel: &Settings, area: Rect) {
    let t = &app.theme;
    let opts = &panel.options;
    let rows = opts.specs().iter().enumerate().map(|(i, spec)| {
        let editing = opts.editing.as_ref().filter(|_| i == panel.cursor);
        let (mark, value, color) = match (&spec.kind, opts.value(spec.flag), editing) {
            (_, _, Some(buf)) => ("▸".into(), format!("{buf}▏"), t.accent),
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
fn draw_categories(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let mut lines: Vec<Line> = Vec::new();
    for rule in &app.rules {
        let mut what = Vec::new();
        if !rule.extensions.is_empty() {
            what.push(rule.extensions.join(", "));
        }
        if !rule.domains.is_empty() {
            what.push(rule.domains.join(", "));
        }
        if let Some(min) = rule.min_size {
            what.push(format!("over {}", crate::utils::parse::human(min)));
        }
        let mut to = Vec::new();
        if let Some(q) = &rule.queue {
            to.push(format!("queue {q}"));
        }
        if let Some(d) = &rule.directory {
            to.push(d.clone());
        }
        if let Some(b) = &rule.backend {
            to.push(b.clone());
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{:<34}", what.join(" + ")), Style::default().fg(t.fg)),
            Span::styled(to.join(" · "), Style::default().fg(t.ok)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::styled(
            "no rules yet — new downloads land in the queue you are viewing",
            Style::default().fg(t.muted),
        ));
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(t.panel).fg(t.fg)).block(
            Block::bordered()
                .border_style(Style::default().fg(t.muted).bg(t.panel))
                .style(Style::default().bg(t.panel))
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

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
    let keys: &[(&str, &str)] = match (panel.tab, panel.options.editing.is_some()) {
        (1, true) => &[("Enter", "set"), ("Esc", "cancel"), ("empty", "unset")],
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
