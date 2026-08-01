use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// How much history is kept. A long crawl is chatty, and nobody debugs from
/// the ten-thousandth line back.
const CAP: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Local `HH:MM:SS`.
    pub at: String,
    pub level: Level,
    pub text: String,
}

fn log() -> &'static Mutex<VecDeque<Entry>> {
    static LOG: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record one line. Called from worker threads as well as the ui, so this is
/// a global rather than something threaded through every call.
pub fn write(level: Level, text: impl Into<String>) {
    let entry = Entry { at: stamp(), level, text: text.into() };
    let Ok(mut log) = log().lock() else { return };
    if log.len() == CAP {
        log.pop_front();
    }
    log.push_back(entry);
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
