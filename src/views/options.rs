use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Padding, Row, Table, TableState};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::options::Options;
use crate::models::option::Kind;
use crate::utils::args;

pub fn draw(f: &mut Frame, app: &App) {
    let Some(panel) = &app.options else { return };
    let t = &app.theme;
    let area = centered(f.area(), 78, 24);
    f.render_widget(Clear, area);

    let [table, footer] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).areas(area);

    let rows = panel.specs().iter().enumerate().map(|(i, spec)| {
        let editing = panel.editing.as_ref().filter(|_| i == panel.cursor);
        let (mark, value, color) = match (&spec.kind, panel.value(spec.flag), editing) {
            (_, _, Some(buf)) => ("▸".into(), format!("{buf}▏"), t.accent),
            (Kind::Flag, Some(_), _) => ("[x]".into(), String::new(), t.ok),
            (Kind::Flag, None, _) => ("[ ]".into(), String::new(), t.muted),
            (Kind::Value, Some(v), _) => ("▸".into(), v.to_string(), t.ok),
            (Kind::Value, None, _) => (" ".to_string(), format!("— {}", spec.hint), t.muted),
        };
        Row::new(vec![
            Cell::from(mark).style(Style::default().fg(color)),
            Cell::from(spec.label),
            Cell::from(spec.flag).style(Style::default().fg(t.muted)),
            Cell::from(value).style(Style::default().fg(color)),
        ])
        .style(Style::default().bg(if i % 2 == 0 { t.panel } else { t.bg }).fg(t.fg))
    });

    let unknown = panel.unknown().len();
    let title = format!(
        " {} options{} ",
        panel.backend,
        if unknown > 0 {
            format!(" · {unknown} extra flag(s) kept")
        } else {
            String::new()
        }
    );

    let widths = [
        Constraint::Length(3),
        Constraint::Min(24),
        Constraint::Length(28),
        Constraint::Length(18),
    ];
    let mut state = TableState::new().with_selected(Some(panel.cursor));
    f.render_stateful_widget(
        Table::new(rows, widths)
            .column_spacing(1)
            .row_highlight_style(Style::default().bg(t.selected).add_modifier(Modifier::BOLD))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.accent).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::horizontal(1))
                    .title(Span::styled(
                        title,
                        Style::default().fg(t.accent).bg(t.panel).add_modifier(Modifier::BOLD),
                    )),
            ),
        table,
        &mut state,
    );

    f.render_widget(
        ratatui::widgets::Paragraph::new(hints(panel, t))
            .style(Style::default().bg(t.panel))
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(t.muted).bg(t.panel))
                    .style(Style::default().bg(t.panel))
                    .padding(Padding::horizontal(1))
                    .title_bottom(Span::styled(
                        format!(" {} ", args::path(panel.backend).display()),
                        Style::default().fg(t.muted).bg(t.panel),
                    )),
            ),
        footer,
    );
}

fn hints<'a>(panel: &Options, t: &crate::views::theme::Theme) -> Line<'a> {
    let keys: &[(&str, &str)] = if panel.editing.is_some() {
        &[("Enter", "set"), ("Esc", "cancel"), ("empty", "unset")]
    } else {
        &[
            ("j/k", "move"),
            ("Enter", "toggle / edit"),
            ("x", "unset"),
            ("Esc", "save & close"),
        ]
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
