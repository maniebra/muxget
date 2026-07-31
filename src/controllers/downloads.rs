use std::process::Stdio;

use crate::controllers::app::App;
use crate::models::download::{Download, Status, Update};
use crate::models::{backend, pick, ytdlp};
use crate::utils::parse::{bytes, for_each_line};

/// Which rows the list shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    All,
    Active,
    Done,
    Failed,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Active, Filter::Done, Filter::Failed];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Active => "active",
            Filter::Done => "done",
            Filter::Failed => "failed",
        }
    }

    pub fn matches(self, status: &Status) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => {
                matches!(status, Status::Running | Status::Queued | Status::Paused)
            }
            Filter::Done => *status == Status::Done,
            Filter::Failed => matches!(status, Status::Failed(_) | Status::Cancelled),
        }
    }
}

/// Adding, starting, stopping and listing downloads.
impl App {
    /// Enqueue a url. Playlists are expanded into one row per entry first, so
    /// each video gets its own progress, slot and cancel.
    pub fn add(&mut self, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        if ytdlp::expands_playlist(url) {
            self.expand_playlist(url);
            return;
        }
        let queue = self.queue().id;
        self.enqueue(url, queue);
    }

    /// Add one url to `queue`. It starts as soon as that queue has a slot.
    pub(crate) fn enqueue(&mut self, url: &str, queue: usize) {
        let Some(backend) = pick(url) else {
            self.message = format!("no backend accepts {url}");
            return;
        };
        let id = self.next_id;
        self.next_id += 1;
        self.downloads.push(Download {
            id,
            queue,
            url: url.to_string(),
            backend: backend.name(),
            status: Status::Queued,
            progress: Default::default(),
            child: None,
        });
        self.message = format!("queued {url}");
        self.pump();
    }

    /// List a playlist's entries off-thread; each one arrives as `Discovered`.
    fn expand_playlist(&mut self, url: &str) {
        let queue = self.queue().id;
        let (tx, url) = (self.tx.clone(), url.to_string());
        self.message = format!("expanding playlist {url}…");

        std::thread::spawn(move || {
            let mut child = match ytdlp::list_command(&url)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Update::Notice(format!("yt-dlp: {e}")));
                    return;
                }
            };
            let stdout = child.stdout.take().expect("piped above");

            let mut found = 0;
            for_each_line(stdout, |line| {
                if line.starts_with("http") {
                    found += 1;
                    let _ = tx.send(Update::Discovered(queue, line.to_string()));
                }
            });
            let _ = child.wait();
            let _ = tx.send(Update::Notice(match found {
                0 => format!("no playlist entries found in {url}"),
                n => format!("queued {n} entries from the playlist"),
            }));
        });
    }

    /// Fill every queue's free slots. Queues run independently of each other.
    pub fn pump(&mut self) {
        for i in 0..self.queues.len() {
            // A paused queue starts nothing, however many slots are free.
            if self.queues[i].paused {
                continue;
            }
            let (id, max) = (self.queues[i].id, self.queues[i].max_active);
            while self.active_in(id) < max {
                let Some(at) = self.next_queued(id) else { break };
                self.start(at);
            }
        }
    }

    fn start(&mut self, at: usize) {
        let (id, url) = match self.downloads.get(at) {
            Some(d) => (d.id, d.url.clone()),
            None => return,
        };
        let Some(backend) = pick(&url) else { return };
        let name = backend.name();
        match backend::run(backend, &url, &self.dir, id, self.tx.clone()) {
            Ok(child) => {
                let d = &mut self.downloads[at];
                d.child = Some(child);
                d.status = Status::Running;
                self.message = format!("started {url}");
            }
            // Failing to spawn must not leave it Queued, or pump would spin.
            Err(e) => {
                self.downloads[at].status = Status::Failed(format!("{name}: {e}"));
                self.message = format!("{name} failed to start: {e}");
            }
        }
    }

    /// Pause a running download, or resume a paused one.
    pub fn toggle_pause(&mut self, at: usize) {
        let Some(d) = self.downloads.get_mut(at) else { return };
        match d.status {
            Status::Running => {
                d.pause();
                self.message = format!("paused {}", self.downloads[at].url);
                // The freed slot goes to whatever is waiting.
                self.pump();
            }
            // ponytail: resuming can put a queue one over its limit until
            // something finishes; give resume its own wait state if that bites.
            Status::Paused => {
                d.resume();
                self.message = format!("resumed {}", self.downloads[at].url);
            }
            _ => {}
        }
    }

    /// Stop the download but keep the row.
    pub fn cancel(&mut self, at: usize) {
        if let Some(d) = self.downloads.get_mut(at) {
            if matches!(d.status, Status::Running | Status::Queued | Status::Paused) {
                d.kill();
                d.status = Status::Cancelled;
            }
        }
        // Cancelling frees a slot for whatever is waiting.
        self.pump();
    }

    /// Restart the download at `at` under a new url.
    pub fn edit(&mut self, at: usize, url: &str) {
        if self.downloads.get(at).is_none() {
            return;
        }
        self.delete(at);
        self.add(url);
    }

    /// Stop and forget the download; the worker thread's later updates are
    /// looked up by id, so removing a row cannot misroute them.
    pub fn delete(&mut self, at: usize) {
        if at >= self.downloads.len() {
            return;
        }
        self.downloads[at].kill();
        self.downloads.remove(at);
        self.clamp_selection();
    }

    pub fn active_in(&self, queue: usize) -> usize {
        self.downloads
            .iter()
            .filter(|d| d.queue == queue && d.status == Status::Running)
            .count()
    }

    pub fn queued_in(&self, queue: usize) -> usize {
        self.downloads
            .iter()
            .filter(|d| d.queue == queue && d.status == Status::Queued)
            .count()
    }

    /// Index of the download that should start next in `queue`, oldest first.
    pub fn next_queued(&self, queue: usize) -> Option<usize> {
        self.downloads
            .iter()
            .position(|d| d.queue == queue && d.status == Status::Queued)
    }

    /// Sum of the running downloads' reported speeds, in bytes/s.
    pub fn speed(&self) -> f64 {
        self.downloads
            .iter()
            .filter(|d| d.status == Status::Running)
            .filter_map(|d| bytes(&d.progress.speed))
            .sum()
    }

    /// Indices of the downloads shown: the current queue, current filter.
    pub fn visible(&self) -> Vec<usize> {
        let queue = self.queue().id;
        self.downloads
            .iter()
            .enumerate()
            .filter(|(_, d)| d.queue == queue && self.filter.matches(&d.status))
            .map(|(i, _)| i)
            .collect()
    }

    /// Move the selection `delta` rows within the visible subset.
    pub fn move_selection(&mut self, delta: isize) {
        let visible = self.visible();
        if visible.is_empty() {
            return;
        }
        let at = visible
            .iter()
            .position(|i| *i == self.selected)
            .unwrap_or(0) as isize;
        let next = (at + delta).clamp(0, visible.len() as isize - 1);
        self.selected = visible[next as usize];
    }

    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        // Keep the selection on a row that is actually shown.
        if !self.visible().contains(&self.selected) {
            self.selected = self.visible().first().copied().unwrap_or(0);
        }
    }

    /// Anything that changes which rows are visible must call this, or the
    /// selection can point at a filtered-out or deleted row.
    pub(crate) fn clamp_selection(&mut self) {
        let visible = self.visible();
        if !visible.contains(&self.selected) {
            self.selected = visible
                .iter()
                .rev()
                .find(|i| **i <= self.selected)
                .or(visible.first())
                .copied()
                .unwrap_or(0);
        }
    }
}
