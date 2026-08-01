use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::controllers::crawl::CRAWL_LABELS;
use crate::controllers::keys::{menu_for, Dialog, Field, Form, Pick, FORM_LABELS, SECRET_FIELDS};
use crate::models::crawl::Found;
use crate::utils::parse::human;
use crate::views::theme::Theme;

/// What a confirmation is about: the marked rows counted, or the one row the
/// cursor is on named.
fn targets(app: &App, at: usize) -> String {
    match app.marked.len() {
        0 => app.downloads.get(at).map_or(String::new(), |d| d.url.clone()),
        1 => app
            .downloads
            .iter()
            .find(|d| app.marked.contains(&d.id))
            .map_or(String::new(), |d| d.url.clone()),
        n => format!("{n} selected downloads"),
    }
}

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
                Line::styled(targets(app, *at), Style::default().fg(t.err)),
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
                    match app.marked.is_empty() {
                        false => targets(app, *at),
                        true => app.downloads.get(*at).map_or(String::new(), |d| match &d.path {
                            Some(p) => p.display().to_string(),
                            None => format!("{} (no file written yet)", d.url),
                        }),
                    },
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
            {
                let mut lines = named(t, "schedule", buf);
                lines.push(Line::default());
                for help in [
                    "22:00-06:00 mon-fri · on=2026-08-01 · once",
                    "sync=6h · retry=3 · quota=150MB/4h",
                    "shutdown · after=<command> (must come last)",
                ] {
                    lines.push(Line::styled(help, Style::default().fg(t.muted)));
                }
                lines
            },
            "Enter save · empty clears · Esc cancel",
        ),
        Dialog::Crawl(form) => (
            "crawl a page",
            crawl_lines(t, form),
            "Tab next · Enter crawl · Esc cancel",
        ),
        Dialog::Crawled(_, found, picked, at) => (
            "discovered links",
            found_lines(t, found, picked, *at),
            "space pick · a all · Enter download · Esc cancel",
        ),
        Dialog::Paste(urls, picked, at) => (
            "paste from the clipboard",
            pick_lines(
                t,
                format!("{} urls found · {} picked", urls.len(), picked.len()),
                urls,
                picked,
                *at,
            ),
            "space pick · a all · Enter add · Esc cancel",
        ),
        Dialog::Playlist(pick) => (
            "playlist entries",
            playlist_lines(t, pick),
            match pick.editing.is_some() {
                true => "Enter apply · Esc keep the old value",
                false => "space pick · a all · / words · t dates · d directory · Enter download",
            },
        ),
        Dialog::QueueClear(at, all) => (
            match all {
                true => "clear the queue",
                false => "clear finished rows",
            },
            vec![
                Line::styled(
                    match all {
                        true => "Remove every row from this queue, stopping whatever is still running? The files they wrote stay on disk.",
                        false => "Remove the done, cancelled and failed rows from this queue? The files they wrote stay on disk.",
                    },
                    Style::default().fg(t.fg),
                ),
                Line::default(),
                Line::styled(
                    format!(
                        "{} rows in {}",
                        app.clearable_in(*at, *all),
                        app.queues.get(*at).map_or("", |q| q.name.as_str())
                    ),
                    Style::default().fg(t.err),
                ),
            ],
            "y/Enter clear · n/Esc keep",
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

    // The add form is taller: its fields plus the preview.
    let area = centered(f.area(), 76, match dialog {
        Dialog::Add(_) => 16,
        Dialog::Crawl(_) => 13,
        Dialog::Crawled(..) | Dialog::Playlist(_) | Dialog::Paste(..) => 20,
        // Its field plus the three lines of spec help.
        Dialog::QueueSchedule(..) => 13,
        _ => 9,
    });
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

/// One line per field; only the focused one shows a caret.
fn form_lines<'a>(t: &Theme, form: &Form) -> Vec<Line<'a>> {
    let hints = [
        "",
        "one item — e.g. 1-10 with %d or %03d in the url",
        "the download directory",
        "the backend's choice",
        "no limit",
        "no login",
        "no login",
    ];
    let mut lines = fields(t, form, &FORM_LABELS, &hints, &SECRET_FIELDS);
    lines.push(Line::default());
    lines.extend(preview(t, form));
    lines
}

/// The crawl form. Same widget, different labels — and no secrets in it.
fn crawl_lines<'a>(t: &Theme, form: &Form) -> Vec<Line<'a>> {
    // An empty field means the crawler tab's default, so say so rather than
    // naming a built-in that the settings may have replaced.
    let hints = [
        "the page to crawl",
        "how many links deep — settings › crawler",
        "e.g. pdf,zip,mp3 — settings › crawler",
        "everything — url patterns, `*` allowed",
        "nothing — url patterns to skip",
        "e.g. 1M-500M — settings › crawler",
        "offline · any-domain/same-domain, under-path/any-path, no-robots/robots, flat/nested",
    ];
    fields(t, form, &CRAWL_LABELS, &hints, &[false; 7])
}

fn fields<'a>(
    t: &Theme,
    form: &Form,
    labels: &[&str; 7],
    hints: &[&str; 7],
    secret: &[bool; 7],
) -> Vec<Line<'a>> {
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let on = i == form.cursor;
            // A password is never drawn, not even for the person typing it.
            let shown_value = if secret[i] {
                "•".repeat(form.fields[i].chars().count())
            } else {
                form.fields[i].clone()
            };
            let value = &shown_value;
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

/// What the form would actually add, so a wrong pattern shows up first.
fn preview<'a>(t: &Theme, form: &Form) -> Vec<Line<'a>> {
    let urls = form.urls();
    if urls.iter().all(|u| u.is_empty()) {
        return vec![];
    }
    let count = urls.len();
    let mut lines = vec![Line::styled(
        if count == 1 {
            "preview".to_string()
        } else {
            format!("preview — {count} downloads")
        },
        Style::default().fg(t.muted),
    )];
    // The ends are what show a bad pattern.
    let shown: Vec<usize> = if count <= 3 {
        (0..count).collect()
    } else {
        vec![0, 1, count - 1]
    };
    for (n, i) in shown.iter().enumerate() {
        if n == 2 && count > 3 {
            lines.push(Line::styled("  …", Style::default().fg(t.muted)));
        }
        lines.push(Line::styled(
            format!("  {}", urls[*i]),
            Style::default().fg(t.fg),
        ));
    }
    if count as i64 == crate::utils::MAX_EXPANSION {
        lines.push(Line::styled(
            format!("  capped at {} items", crate::utils::MAX_EXPANSION),
            Style::default().fg(t.err),
        ));
    }
    lines
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

/// The links a crawl found: what is picked, how big it is, and a running
/// total, so the choice is made before anything is fetched.
fn found_lines<'a>(t: &Theme, found: &[Found], picked: &[usize], at: usize) -> Vec<Line<'a>> {
    let total: f64 = picked.iter().filter_map(|i| found.get(*i)?.size).sum();
    let head = format!("{} links found · {} picked · {}", found.len(), picked.len(), human(total));
    let rows: Vec<String> = found
        .iter()
        .map(|f| format!("{:>9}  {}", f.size.map(human).unwrap_or_default(), f.url))
        .collect();
    pick_lines(t, head, &rows, picked, at)
}

fn playlist_lines<'a>(t: &Theme, pick: &Pick) -> Vec<Line<'a>> {
    let shown = pick.shown();
    // The field being typed into shows its cursor; the rest show their value.
    let field = |which: Field, label: &str, value: &str, empty: &'static str| match pick.editing {
        Some((f, ref buf)) if f == which => format!("{label}: {buf}▏"),
        _ => format!("{label}: {}", if value.is_empty() { empty } else { value }),
    };
    let head = format!(
        "{} entries · {} shown · {} picked\n{}\n{}  ·  {}",
        pick.listing.entries.len(),
        shown.len(),
        pick.picked.len(),
        field(Field::Dir, "directory", &pick.listing.over.dir, "(the download directory)"),
        field(Field::Words, "words", &pick.words, "(any title)"),
        field(Field::Dates, "uploaded", &pick.listing.dates.typed(), "(any date)"),
    );
    // The title if yt-dlp knew one, the url otherwise.
    let rows: Vec<String> = shown
        .iter()
        .map(|i| {
            let (url, title) = &pick.listing.entries[*i];
            match title.is_empty() {
                true => url.clone(),
                false => title.clone(),
            }
        })
        .collect();
    // `picked` counts entries, the rows on screen are the shown ones.
    let picked: Vec<usize> = shown
        .iter()
        .enumerate()
        .filter(|(_, entry)| pick.picked.contains(entry))
        .map(|(row, _)| row)
        .collect();
    pick_lines(t, head, &rows, &picked, pick.at)
}

/// A pick-from-a-list body: a header, then one `[x] row` per entry with the
/// cursor kept on screen — no scroll state of its own.
fn pick_lines<'a>(t: &Theme, head: String, rows: &[String], picked: &[usize], at: usize) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = head
        .lines()
        .map(|l| Line::styled(l.to_string(), Style::default().fg(t.accent)))
        .collect();
    lines.push(Line::default());
    let capacity = 12 - (lines.len() - 2);
    let from = at.saturating_sub(capacity.saturating_sub(1));
    for (i, row) in rows.iter().enumerate().skip(from).take(capacity) {
        let on = i == at;
        let mark = if picked.contains(&i) { "[x]" } else { "[ ]" };
        lines.push(Line::styled(
            format!("{} {} {}", if on { "▸" } else { " " }, mark, row),
            Style::default()
                .fg(if on { t.accent } else { t.fg })
                .add_modifier(if on { Modifier::BOLD } else { Modifier::empty() }),
        ));
    }
    lines
}
