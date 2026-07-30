use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::controllers::app::{App, Dialog};
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
    let Some(dialog) = &app.dialog else {
        return;
    };
    let t = &app.theme;
    let (title, body, hint) = match dialog {
        Dialog::Add(buf) => ("add download", field(t, buf), "Enter add · Esc cancel"),
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

fn field<'a>(t: &Theme, buf: &str) -> Vec<Line<'a>> {
    vec![
        Line::styled("url", Style::default().fg(t.muted)),
        Line::from(vec![
            Span::styled("▸ ", Style::default().fg(t.accent)),
            Span::styled(
                format!("{buf}▏"),
                Style::default().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
        ]),
    ]
}
