use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::controllers::app::App;
use crate::views::dialog::centered;

/// A page of the manual: its tab name and its lines. A line is either
/// `key\tmeaning`, which is laid out as a key table, or prose.
pub struct Page {
    pub name: &'static str,
    pub lines: &'static [&'static str],
}

pub const PAGES: &[Page] = &[
    Page {
        name: "basics",
        lines: &[
            "muxget drives aria2c, yt-dlp and wget. A url goes to whichever of them",
            "claims it, and every download is a row in a queue.",
            "",
            "a\tadd a url, with per-download settings",
            "v\tadd every url in the clipboard, after a preview",
            "c\tcrawl a page for links",
            "e\tedit the selected url — this restarts it",
            "Tab / f\tcycle filter: all / active / done / failed",
            "s\tsettings",
            "?\tthis manual",
            "q / ZZ\tquit",
        ],
    },
    Page {
        name: "moving",
        lines: &[
            "The list takes vim's movements, and a count repeats them: 5j, 12G.",
            "",
            "j / k\tdown / up one row",
            "gg / G\tfirst / last row — 5G is the fifth",
            "Ctrl-d / Ctrl-u\thalf a screen down / up",
            "Ctrl-f / Ctrl-b\ta whole screen down / up",
            "[ / ]\tprevious / next queue",
            "J / K\tmove the download within its queue",
            "",
            "The mouse works too: click a queue, a filter or a row, and scroll over",
            "the list or the queue sidebar.",
        ],
    },
    Page {
        name: "select",
        lines: &[
            "Space\tselect or deselect the row under the cursor",
            "M\tselect from the last selected row to the cursor",
            "A\tselect every row on screen, or none if they all are",
            "",
            "Every per-row command then acts on the selection — p, x, d, it, iR.",
            "With nothing selected they act on the row under the cursor, so this is",
            "something you opt into.",
            "",
            "Selections are download ids, not row numbers: they survive filtering,",
            "reordering and deleting several rows at once. A command clears the",
            "selection when it is done.",
        ],
    },
    Page {
        name: "items",
        lines: &[
            "p\tpause or resume",
            "x\tstop, keeping the row",
            "d / Del\tdelete (asks first)",
            "ir / iR\tremove — iR deletes the file too",
            "it\tretry a failed or cancelled download",
            "io / if\topen the file / its folder",
            "iF\tforce restart a stalled torrent",
            "",
            "A pause keeps the process and its partial file; the slot it held goes to",
            "whatever is waiting. A failed download is retried up to its queue's",
            "retry limit on its own; `it` retries it past that limit.",
        ],
    },
    Page {
        name: "queues",
        lines: &[
            "Each queue runs its own downloads, up to its own slot count.",
            "",
            "gn / gr / gd\tnew / rename / delete queue",
            "gc\tclear the finished rows",
            "C / gC\tclear every row (asks first)",
            "gp / gP\tpause this queue / every queue",
            "gt\tschedule: window, days, sync, retry, quota",
            "g+ / g-\tone more / one less slot",
            "g> / g<\tmove this queue in the tab order",
            "",
            "A schedule reads like 22:00-06:00 mon-fri retry=3 quota=150MB/4h.",
            "Clearing never deletes files; only iR does that.",
        ],
    },
    Page {
        name: "lists",
        lines: &[
            "A playlist, channel or mix expands into one row per video.",
            "",
            "Turn on `confirm before dl playlist` in settings and it opens a picker",
            "instead:",
            "",
            "Space / a\tpick a row / all of them",
            "/\tfilter by words in the title — -word excludes, * globs",
            "t / T\tuploaded from / to — dates are approximate, and filter on screen",
            "d\tthe directory they all land in",
            "Enter\tqueue what is picked",
            "",
            "A date range costs time: upload dates are not in a playlist's index, so",
            "yt-dlp has to open every entry to read one.",
        ],
    },
    Page {
        name: "crawl",
        lines: &[
            "c walks a site and comes back with the links it found, to pick from.",
            "Space picks, a picks all, Enter queues.",
            "",
            "The form's fields default to settings > crawler when left empty. Its",
            "options field takes words, each with an opposite:",
            "",
            "any-domain / same-domain\tfollow links off the host",
            "under-path / any-path\tstay at or below the start url",
            "no-robots / robots\tignore robots.txt and nofollow",
            "flat / nested\tsave without the directory tree",
            "offline\tmirror the site for reading offline instead",
            "",
            "Media often lives on another host — a CDN or an archive — which is what",
            "any-domain is for.",
        ],
    },
    Page {
        name: "settings",
        lines: &[
            "s opens the panel; Tab moves between its tabs.",
            "",
            "general\ttheme, download directory, icons, playlist confirmation",
            "backends\taria2c, yt-dlp and wget flags — b switches backend",
            "crawler\tthe defaults the crawl form opens with",
            "categories\trouting rules: n new, Enter edit, x clear or delete",
            "log\twhat every backend did and said — x clears it",
            "",
            "A rule sends matching urls to a queue, a directory or a backend. Its",
            "pattern field captures each * for $1, $2 in those, so one rule can give",
            "youtube.com/@* a directory per channel.",
            "",
            "Everything is a plain file under ~/.config/muxget, editable by hand.",
        ],
    },
];

pub fn draw(f: &mut Frame, app: &App) {
    let Some(help) = &app.help else { return };
    let t = &app.theme;
    let area = centered(f.area(), 92, 26);
    f.render_widget(Clear, area);

    let [tabs, body] = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);
    draw_tabs(f, app, help.tab, tabs);

    let page = &PAGES[help.tab.min(PAGES.len() - 1)];
    let lines: Vec<Line> = page
        .lines
        .iter()
        .skip(help.scroll)
        .map(|line| match line.split_once('\t') {
            // A key and what it does, in two columns.
            Some((key, meaning)) => Line::from(vec![
                Span::styled(
                    format!("  {key:<22}"),
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(meaning.to_string(), Style::default().fg(t.fg)),
            ]),
            None => Line::styled(format!("  {line}"), Style::default().fg(t.muted)),
        })
        .collect();

    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).style(Style::default().bg(t.panel)).block(
            Block::bordered()
                .border_style(Style::default().fg(t.accent).bg(t.panel))
                .style(Style::default().bg(t.panel))
                .padding(Padding::uniform(1))
                .title_bottom(Span::styled(
                    " Tab / h l next page · j k scroll · Esc close ",
                    Style::default().fg(t.muted).bg(t.panel),
                )),
        ),
        body,
    );
}

fn draw_tabs(f: &mut Frame, app: &App, tab: usize, area: Rect) {
    let t = &app.theme;
    let mut spans = Vec::new();
    for (i, page) in PAGES.iter().enumerate() {
        spans.push(Span::styled(
            format!(" {} ", page.name),
            match i == tab {
                true => Style::default().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD),
                false => Style::default().fg(t.muted).bg(t.panel),
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
                    " manual ",
                    Style::default().fg(t.accent).bg(t.panel).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}
