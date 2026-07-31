use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::keys::{menu_for, Dialog, Form, FORM_LABELS};
use crate::views::theme::Theme;

/// Centered popover, at most `w` x `h` but never wider than the terminal.
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let [row] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::horizontal([Constraint::Length(w)])
        .flex(Flex::Center)
        .areas(row);
    cell
}

pub fn draw(f: &mut Frame, app: &App) {
    if let Some(prefix) = app.pending {
        draw_menu(f, app, prefix);
        return;
    }
    let Some(dialog) = &app.dialog else {
        return;
    };
    let t = &app.theme;
    let (title, body, hint) = match dialog {
        Dialog::Add(form) => (
            "add download",
            form_lines(t, form),
            "Tab next · Enter add · Esc cancel",
        ),
        Dialog::Edit(_, buf) => ("edit url", field(t, buf), "Enter restart · Esc cancel"),
        Dialog::Delete(at) => (
            "delete download",
            vec![
                Line::styled(
                    "Remove this download? The running transfer is stopped.",
                    Style::default().fg(t.fg),
                ),
                Line::default(),
                Line::styled(
                    app.downloads.get(*at).map_or(String::new(), |d| d.url.clone()),
                    Style::default().fg(t.err),
                ),
            ],
            "y/Enter delete · n/Esc keep",
        ),
        Dialog::DeleteData(at) => (
            "delete download and file",
            vec![
                Line::styled(
                    "Remove this download and delete what it wrote to disk?",
                    Style::default().fg(t.fg),
                ),
                Line::default(),
                Line::styled(
                    app.downloads
                        .get(*at)
                        .map_or(String::new(), |d| match &d.path {
                            Some(p) => p.display().to_string(),
                            None => format!("{} (no file written yet)", d.url),
                        }),
                    Style::default().fg(t.err),
                ),
            ],
            "y/Enter delete · n/Esc keep",
        ),
        Dialog::SetDir(buf) => (
            "download directory",
            named(t, "path", buf),
            "Enter save · Esc cancel",
        ),
        Dialog::QueueNew(buf) => (
            "new queue",
            named(t, "name", buf),
            "Enter create · Esc cancel",
        ),
        Dialog::QueueRename(_, buf) => (
            "rename queue",
            named(t, "name", buf),
            "Enter rename · Esc cancel",
        ),
        Dialog::QueueSchedule(_, buf) => (
            "queue schedule",
            named(t, "window", buf),
            "Enter save · empty clears · Esc cancel",
        ),
        Dialog::QueueDelete(at) => (
            "delete queue",
            vec![
                Line::styled(
                    "Delete this queue? Its downloads move to the default queue.",
                    Style::default().fg(t.fg),
                ),
                Line::default(),
                Line::styled(
                    app.queues.get(*at).map_or(String::new(), |q| q.name.clone()),
                    Style::default().fg(t.err),
                ),
            ],
            "y/Enter delete · n/Esc keep",
        ),
    };

    let area = centered(f.area(), 76, 9);
    // Clear so the list underneath does not bleed through the popover.
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(t.panel).fg(t.fg))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.accent).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::uniform(1))
                    .title(Span::styled(
                        format!(" {title} "),
                        Style::default().fg(t.accent).bg(t.panel).add_modifier(Modifier::BOLD),
                    ))
                    .title_bottom(Span::styled(
                        format!(" {hint} "),
                        Style::default().fg(t.muted).bg(t.panel),
                    )),
            ),
        area,
    );
}

/// which-key popover: the keys that continue the half-typed sequence.
fn draw_menu(f: &mut Frame, app: &App, prefix: char) {
    let t = &app.theme;
    let Some((name, items)) = menu_for(prefix) else {
        return;
    };

    let lines: Vec<Line> = items
        .iter()
        .map(|item| {
            Line::from(vec![
                Span::styled(
                    format!(" {} ", item.key),
                    Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", item.label), Style::default().fg(t.fg)),
            ])
        })
        .collect();

    let area = centered(f.area(), 40, items.len() as u16 + 4);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(t.panel).fg(t.fg))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.accent).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::horizontal(1))
                    .title(Span::styled(
                        format!(" {prefix} — {name} "),
                        Style::default().fg(t.accent).bg(t.panel).add_modifier(Modifier::BOLD),
                    ))
                    .title_bottom(Span::styled(
                        " any other key cancels ",
                        Style::default().fg(t.muted).bg(t.panel),
                    )),
            ),
        area,
    );
}

fn field<'a>(t: &Theme, buf: &str) -> Vec<Line<'a>> {
    named(t, "url", buf)
}

/// One line per field; only the focused one shows a caret. The three override
/// fields say what leaving them empty means, so the form documents itself.
fn form_lines<'a>(t: &Theme, form: &Form) -> Vec<Line<'a>> {
    let hints = ["", "the download directory", "the backend's choice", "no limit"];
    FORM_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let on = i == form.cursor;
            let value = &form.fields[i];
            let shown = match (on, value.is_empty()) {
                (true, _) => format!("{value}▏"),
                (false, true) => format!("— {}", hints[i]),
                (false, false) => value.clone(),
            };
            Line::from(vec![
                Span::styled(
                    format!("{:>11} ", label),
                    Style::default().fg(if on { t.accent } else { t.muted }),
                ),
                Span::styled(
                    shown,
                    Style::default()
                        .fg(if value.is_empty() && !on { t.muted } else { t.fg })
                        .add_modifier(if on { Modifier::BOLD } else { Modifier::empty() }),
                ),
            ])
        })
        .collect()
}

fn named<'a>(t: &Theme, label: &str, buf: &str) -> Vec<Line<'a>> {
    vec![
        Line::styled(label.to_string(), Style::default().fg(t.muted)),
        Line::from(vec![
            Span::styled("▸ ", Style::default().fg(t.accent)),
            Span::styled(
                format!("{buf}▏"),
                Style::default().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
        ]),
    ]
}
