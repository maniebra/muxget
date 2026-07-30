use std::path::PathBuf;

use muxget::controllers::app::App;
use muxget::views::theme::Theme;

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // -d DIR, else $PWD.
    let dir = match args.iter().position(|a| a == "-d") {
        Some(i) if i + 1 < args.len() => PathBuf::from(args.drain(i..i + 2).nth(1).unwrap()),
        _ => std::env::current_dir()?,
    };

    let theme = match args.iter().position(|a| a == "--theme") {
        Some(i) if i + 1 < args.len() => args.drain(i..i + 2).nth(1).unwrap(),
        _ => std::env::var("MUXGET_THEME").unwrap_or_default(),
    };

    // -j N concurrent downloads.
    let jobs = match args.iter().position(|a| a == "-j") {
        Some(i) if i + 1 < args.len() => args.drain(i..i + 2).nth(1).and_then(|n| n.parse().ok()),
        _ => None,
    };

    let mut app = App::new(dir);
    if let Some(jobs) = jobs {
        app.set_max_active(jobs);
    }
    if !theme.is_empty() {
        app.theme = Theme::named(&theme);
    }
    for url in &args {
        app.add(url);
    }

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
