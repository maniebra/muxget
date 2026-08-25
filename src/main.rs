use std::path::PathBuf;

use muxget::controllers::app::App;
use muxget::models::log;
use muxget::models::state::State;
use muxget::utils::config_dir;
use muxget::views::theme::Theme;

/// `--flag value`, removed from `args` so what is left is urls. Falls back to
/// an environment variable, which is how a shell profile sets a default.
fn opt(args: &mut Vec<String>, flag: &str, env: &str) -> Option<String> {
    match args.iter().position(|a| a == flag) {
        Some(i) if i + 1 < args.len() => args.drain(i..i + 2).nth(1),
        _ => std::env::var(env).ok().filter(|v| !v.is_empty()),
    }
}

/// Logging as the flags asked for it: `--log <file|off>`, `--log-level
/// <debug|info|warn|error>`, `--log-format "<template>"`. Written to
/// `<config>/muxget.log` unless told otherwise.
fn logging(args: &mut Vec<String>) -> log::Config {
    let file = match opt(args, "--log", "MUXGET_LOG") {
        Some(v) if v.trim().eq_ignore_ascii_case("off") => None,
        Some(v) => Some(PathBuf::from(muxget::utils::expand_home(v.trim()))),
        None => Some(config_dir().join("muxget.log")),
    };
    let level = opt(args, "--log-level", "MUXGET_LOG_LEVEL")
        .and_then(|v| log::Level::named(&v))
        .unwrap_or(log::Level::Info);
    let format = opt(args, "--log-format", "MUXGET_LOG_FORMAT")
        .unwrap_or_else(|| log::DEFAULT_FORMAT.to_string());
    log::Config { level, file, format }
}

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    log::configure(logging(&mut args));

    // -d DIR wins, then the directory saved last run, then $PWD.
    let dir = match opt(&mut args, "-d", "") {
        Some(dir) => PathBuf::from(dir),
        None => State::load().dir.unwrap_or(std::env::current_dir()?),
    };

    let theme = opt(&mut args, "--theme", "MUXGET_THEME").unwrap_or_default();

    // -j N concurrent downloads.
    let jobs: Option<usize> = opt(&mut args, "-j", "").and_then(|n| n.parse().ok());

    let mut app = App::new(dir);
    if let Some(jobs) = jobs {
        // Per-run override, so set it directly rather than persisting it.
        app.queues[0].max_active = jobs.clamp(1, 16);
    }
    if !theme.is_empty() {
        app.theme = Theme::named(&theme);
    }
    log::debug(format!("muxget {} starting in {}", env!("CARGO_PKG_VERSION"), app.dir.display()));
    for url in &args {
        app.add(url);
    }

    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
    let result = app.run(&mut terminal);
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    log::debug("muxget exiting");
    result
}
