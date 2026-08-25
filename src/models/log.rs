use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// How much history is kept. A long crawl is chatty, and nobody debugs from
/// the ten-thousandth line back.
const CAP: usize = 500;

/// A log file bigger than this is rotated to `<name>.1` at startup, so a
/// long-running install does not grow one forever.
const MAX_FILE: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    /// Word as it is written to the file and typed on the command line.
    pub fn word(self) -> &'static str {
        match self {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }

    /// One column in the log tab.
    pub fn symbol(self) -> &'static str {
        match self {
            Level::Debug => "·",
            Level::Info => " ",
            Level::Warn => "!",
            Level::Error => "✗",
        }
    }

    /// `debug|info|warn|error`, however it is capitalised. Anything else is
    /// not a level, and the caller decides what to do about that.
    pub fn named(word: &str) -> Option<Level> {
        match word.trim().to_ascii_lowercase().as_str() {
            "debug" | "trace" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warn" | "warning" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Local `HH:MM:SS`.
    pub at: String,
    pub level: Level,
    pub text: String,
}

/// How much is logged, where a copy goes, and what a written line looks like.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Nothing below this is recorded at all.
    pub level: Level,
    /// File a copy of every line is appended to, if any.
    pub file: Option<PathBuf>,
    /// Line template: `{date} {time} {level} {text}`.
    pub format: String,
}

pub const DEFAULT_FORMAT: &str = "{date} {time} {level} {text}";

impl Default for Config {
    fn default() -> Self {
        Config { level: Level::Info, file: None, format: DEFAULT_FORMAT.to_string() }
    }
}

fn log() -> &'static Mutex<VecDeque<Entry>> {
    static LOG: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(VecDeque::new()))
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn config() -> &'static Config {
    static FALLBACK: OnceLock<Config> = OnceLock::new();
    CONFIG.get().unwrap_or_else(|| FALLBACK.get_or_init(Config::default))
}

/// Settle the configuration for this run. Called once from `main`, before
/// anything is logged; a second call is ignored.
pub fn configure(config: Config) {
    if let Some(path) = &config.file {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Rotate rather than truncate: the previous run is usually the one
        // being asked about.
        if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_FILE) {
            let _ = std::fs::rename(path, path.with_extension("1"));
        }
    }
    let _ = CONFIG.set(config);
}

/// One line as the file gets it, with the configured template filled in.
pub fn render(entry: &Entry, format: &str) -> String {
    format
        .replace("{date}", &date())
        .replace("{time}", &entry.at)
        .replace("{level}", entry.level.word())
        .replace("{text}", &entry.text)
}

/// Record one line. Called from worker threads as well as the ui, so this is
/// a global rather than something threaded through every call.
pub fn write(level: Level, text: impl Into<String>) {
    let config = config();
    if level < config.level {
        return;
    }
    let entry = Entry { at: stamp(), level, text: text.into() };
    if let Some(path) = &config.file {
        append(path, &render(&entry, &config.format));
    }
    let Ok(mut log) = log().lock() else { return };
    if log.len() == CAP {
        log.pop_front();
    }
    log.push_back(entry);
}

/// Best effort, and reopened per line: a log is not worth failing a download
/// over, and the traffic is a handful of lines a second at worst.
// ponytail: open-per-line, keep a handle if the log ever gets chatty.
fn append(path: &PathBuf, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

pub fn debug(text: impl Into<String>) {
    write(Level::Debug, text);
}

pub fn info(text: impl Into<String>) {
    write(Level::Info, text);
}

pub fn warn(text: impl Into<String>) {
    write(Level::Warn, text);
}

pub fn error(text: impl Into<String>) {
    write(Level::Error, text);
}

/// Everything kept, oldest first.
pub fn entries() -> Vec<Entry> {
    log().lock().map(|log| log.iter().cloned().collect()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut log) = log().lock() {
        log.clear();
    }
}

/// Today as `YYYY-MM-DD`, for file lines — the tab shows one session, but a
/// file outlives midnight.
fn date() -> String {
    let today = crate::utils::today();
    match today.len() == 8 {
        true => format!("{}-{}-{}", &today[..4], &today[4..6], &today[6..]),
        false => today,
    }
}

/// Local `HH:MM:SS`. The offset comes from `date(1)` once — the clock itself
/// is the system's, so a log line costs no process.
fn stamp() -> String {
    let offset = *OFFSET.get_or_init(local_offset);
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "--:--:--".into();
    };
    let secs = now.as_secs() as i64 + offset;
    let day = secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

static OFFSET: OnceLock<i64> = OnceLock::new();

/// Seconds east of UTC, as `date +%z` gives it (`+0330`, `-0800`).
fn local_offset() -> i64 {
    let Ok(out) = std::process::Command::new("date").arg("+%z").output() else {
        return 0;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let text = text.trim();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.trim_start_matches('+')),
    };
    if digits.len() < 4 {
        return 0;
    }
    let (h, m) = digits.split_at(2);
    match (h.parse::<i64>(), m[..2].parse::<i64>()) {
        (Ok(h), Ok(m)) => sign * (h * 3600 + m * 60),
        _ => 0,
    }
}
