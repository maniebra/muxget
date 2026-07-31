use std::path::{Path, PathBuf};

use crate::models::download::{Download, Overrides, Status};
use crate::models::queue::{self, Queue, DEFAULT};
use crate::utils::config_dir;

/// What survives a restart: the download directory, the queues, and the list
/// of downloads. Pause state does not — a restart starts clear.
#[derive(Debug, Default, PartialEq)]
pub struct State {
    pub dir: Option<PathBuf>,
    pub queues: Vec<Queue>,
    pub downloads: Vec<SavedDownload>,
}

/// A download as it survives a restart: where it belongs, how it ended, how
/// far it got, and what to fetch. Percent is saved so a restored row shows
/// where it stood instead of 0 until the backend reports again.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedDownload {
    pub queue: usize,
    pub status: Status,
    pub percent: f32,
    pub url: String,
    pub over: Overrides,
}

fn path() -> PathBuf {
    config_dir().join("state")
}

/// Anything unfinished comes back queued, so it resumes rather than claiming
/// to be running a process that died with the last session.
fn status_from(word: &str) -> Status {
    match word {
        "done" => Status::Done,
        "cancelled" => Status::Cancelled,
        "failed" => Status::Failed("failed in a previous run".into()),
        _ => Status::Queued,
    }
}

fn status_word(status: &Status) -> &'static str {
    match status {
        Status::Done => "done",
        Status::Cancelled => "cancelled",
        Status::Failed(_) => "failed",
        // Running, Paused and Queued all resume as queued.
        _ => "queued",
    }
}

impl State {
    pub fn load() -> State {
        State::parse(&std::fs::read_to_string(path()).unwrap_or_default())
    }

    /// `dir = <path>`, `queue = <name>|<slots>|<window>`,
    /// `download = <queue>|<status>|<percent>|<url>`, optionally followed by
    /// `over = <dir>|<name>|<rate>` for the download above it — its own line,
    /// so a url with a `|` in it stays the last unsplit field.
    /// A malformed line is skipped rather than losing the whole file.
    pub fn parse(text: &str) -> State {
        let mut state = State::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "dir" if !value.is_empty() => state.dir = Some(PathBuf::from(value)),
                "queue" => {
                    let (name, rest) = value.split_once('|').unwrap_or((value, "3"));
                    if name.trim().is_empty() {
                        continue;
                    }
                    // The window is optional, so files written before it exist.
                    let (slots, window) = rest.split_once('|').unwrap_or((rest, ""));
                    // Ids are positional: the first queue is always the default.
                    let id = state.queues.len();
                    let mut q = Queue::new(id, name.trim(), slots.trim().parse().unwrap_or(3));
                    q.schedule = queue::parse_window(window);
                    state.queues.push(q);
                }
                "download" => {
                    let Some((queue, rest)) = value.split_once('|') else {
                        continue;
                    };
                    // Url last and unsplit, so one containing `|` survives.
                    let Some((status, rest)) = rest.split_once('|') else {
                        continue;
                    };
                    // Percent is optional: files written before it existed go
                    // straight to the url, which never parses as a number.
                    let (percent, url) = match rest.split_once('|') {
                        Some((p, u)) => match p.trim().parse::<f32>() {
                            Ok(p) => (p, u),
                            Err(_) => (0.0, rest),
                        },
                        None => (0.0, rest),
                    };
                    if url.trim().is_empty() {
                        continue;
                    }
                    state.downloads.push(SavedDownload {
                        queue: queue.trim().parse().unwrap_or(DEFAULT),
                        status: status_from(status.trim()),
                        percent: percent.clamp(0.0, 100.0),
                        url: url.trim().to_string(),
                        over: Overrides::default(),
                    });
                }
                // Attaches to the download above it; a stray one is ignored.
                "over" => {
                    let Some(last) = state.downloads.last_mut() else { continue };
                    let mut parts = value.splitn(3, '|');
                    last.over = Overrides {
                        dir: parts.next().unwrap_or_default().trim().to_string(),
                        name: parts.next().unwrap_or_default().trim().to_string(),
                        rate: parts.next().unwrap_or_default().trim().to_string(),
                    };
                }
                _ => {}
            }
        }
        state
    }

    pub fn render(dir: &Path, queues: &[Queue], downloads: &[Download]) -> String {
        let mut text = format!("dir = {}\n", dir.display());
        for q in queues {
            text.push_str(&format!("queue = {}|{}|{}\n", q.name, q.max_active, q.window()));
        }
        for d in downloads {
            text.push_str(&format!(
                "download = {}|{}|{}|{}\n",
                d.queue,
                status_word(&d.status),
                d.progress.percent,
                d.url
            ));
            if !d.over.is_empty() {
                text.push_str(&format!(
                    "over = {}|{}|{}\n",
                    d.over.dir, d.over.name, d.over.rate
                ));
            }
        }
        text
    }

    /// Best effort: a config that cannot be written is not worth interrupting
    /// a download for.
    pub fn save(dir: &Path, queues: &[Queue], downloads: &[Download]) {
        let _ = std::fs::create_dir_all(config_dir());
        let _ = std::fs::write(path(), State::render(dir, queues, downloads));
    }

    /// Saved queues, or the single default queue when there is nothing saved.
    pub fn queues_or_default(&self) -> Vec<Queue> {
        if self.queues.is_empty() {
            vec![Queue::new(DEFAULT, "default", 3)]
        } else {
            self.queues.clone()
        }
    }
}
