use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::models::download::Status;

pub fn draw(f: &mut Frame, app: &App) {
    let [list, gauge, status] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .areas(f.area());

    let items: Vec<ListItem> = app
        .downloads
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let state = match &d.status {
                Status::Running => format!("{:>5.1}% {} ETA {}", d.progress.percent, d.progress.speed, d.progress.eta),
                Status::Done => "done".into(),
                Status::Cancelled => "cancelled".into(),
                Status::Failed(e) => format!("failed: {e}"),
            };
            let line = format!("{} [{}] {} — {}", if i == app.selected { ">" } else { " " }, d.backend, d.url, state);
            ListItem::new(line)
        })
        .collect();
    f.render_widget(
        List::new(items).block(Block::bordered().title(format!(" muxget — {} ", app.dir.display()))),
        list,
    );

    let current = app.downloads.get(app.selected);
    f.render_widget(
        Gauge::default()
            .block(Block::bordered().title(" progress "))
            .ratio((current.map_or(0.0, |d| d.progress.percent) as f64 / 100.0).clamp(0.0, 1.0)),
        gauge,
    );

    let bottom = match &app.input {
        Some(buf) => Paragraph::new(format!("url: {buf}▏"))
            .block(Block::bordered().title(" add (Enter to start, Esc to cancel) "))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        None => Paragraph::new(app.message.as_str())
            .block(Block::bordered().title(" a add · x cancel · j/k select · q quit ")),
    };
    f.render_widget(bottom, status);
}
