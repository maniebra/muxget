use std::process::Stdio;

use crate::controllers::app::App;
use crate::models::download::{Download, Overrides, Progress, Status, Update};
use crate::models::state::SavedDownload;
use crate::models::{aria2, backend, pick, queue, ytdlp};
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
        self.add_with(url, Overrides::default());
    }

    /// Enqueue a url with its own settings; a playlist passes them to every
    /// entry it expands into.
    pub fn add_with(&mut self, url: &str, over: Overrides) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        if ytdlp::expands_playlist(url) {
            self.expand_playlist(url, over);
            return;
        }
        let queue = self.queue().id;
        self.enqueue(url, queue, over);
    }

    /// Add one url to `queue`. It starts as soon as that queue has a slot.
    pub(crate) fn enqueue(&mut self, url: &str, queue: usize, over: Overrides) {
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
            over,
            status: Status::Queued,
            progress: Default::default(),
            path: None,
            child: None,
        });
        self.message = format!("queued {url}");
        self.save_state();
        self.pump();
    }

    /// List a playlist's entries off-thread; each one arrives as `Discovered`.
    fn expand_playlist(&mut self, url: &str, over: Overrides) {
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
                    let _ = tx.send(Update::Discovered(queue, line.to_string(), over.clone()));
                }
            });
            let _ = child.wait();
            let _ = tx.send(Update::Notice(match found {
                0 => format!("no playlist entries found in {url}"),
                n => format!("queued {n} entries from the playlist"),
            }));
        });
    }

    /// Rebuild last session's list. Ids are reassigned — the saved ones meant
    /// nothing to the processes that are now gone. Unfinished rows come back
    /// queued and resume from their partial files.
    pub fn restore(&mut self, saved: &[SavedDownload]) {
        let known: Vec<usize> = self.queues.iter().map(|q| q.id).collect();
        for s in saved {
            let Some(backend) = pick(&s.url) else { continue };
            let id = self.next_id;
            self.next_id += 1;
            self.downloads.push(Download {
                id,
                // A queue that no longer exists would strand the row.
                queue: if known.contains(&s.queue) { s.queue } else { queue::DEFAULT },
                url: s.url.clone(),
                backend: backend.name(),
                over: s.over.clone(),
                status: s.status.clone(),
                progress: Progress {
                    // A finished row is at 100 whatever the last report said.
                    percent: if s.status == Status::Done { 100.0 } else { s.percent },
                    ..Default::default()
                },
                path: None,
                child: None,
            });
        }
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
        let (id, url, over) = match self.downloads.get(at) {
            Some(d) => (d.id, d.url.clone(), d.over.clone()),
            None => return,
        };
        let Some(backend) = pick(&url) else { return };
        let name = backend.name();
        match backend::run(backend, &url, &self.dir, &over, id, self.tx.clone()) {
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

    /// Stop a torrent and start it again at once, ignoring the queue's slot
    /// limit and any pause — the swarm is what a stuck torrent needs, not a
    /// place in the queue.
    pub fn force_restart(&mut self, at: usize) {
        let Some(d) = self.downloads.get_mut(at) else { return };
        if !aria2::is_torrent(&d.url) {
            self.message = "force restart is for torrents".into();
            return;
        }
        d.kill();
        d.progress = Default::default();
        d.status = Status::Queued;
        self.start(at);
        self.message = format!("force restarted {}", self.downloads[at].url);
    }

    /// Stop the download but keep the row.
    pub fn cancel(&mut self, at: usize) {
        if let Some(d) = self.downloads.get_mut(at) {
            if matches!(d.status, Status::Running | Status::Queued | Status::Paused) {
                d.kill();
                crate::utils::clear_creds(d.id);
                d.status = Status::Cancelled;
            }
        }
        self.save_state();
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
        crate::utils::clear_creds(self.downloads[at].id);
        self.downloads.remove(at);
        self.clamp_selection();
        self.save_state();
    }

    /// Remove the row and delete what it downloaded, if a backend named a file.
    pub fn delete_with_data(&mut self, at: usize) {
        let Some(d) = self.downloads.get(at) else { return };
        let path = d.path.clone();
        self.delete(at);
        self.message = match path {
            // Partial transfers leave a sidecar next to the file.
            Some(p) => match std::fs::remove_file(&p) {
                Ok(()) => {
                    for ext in ["part", "aria2"] {
                        let _ = std::fs::remove_file(p.with_extension(ext));
                    }
                    format!("deleted {}", p.display())
                }
                Err(e) => format!("could not delete {}: {e}", p.display()),
            },
            None => "removed; no file was written yet".into(),
        };
    }

    /// Hand the file to the desktop, or the download directory if unknown.
    pub fn open_item(&mut self, at: usize) {
        let target = match self.downloads.get(at).and_then(|d| d.path.clone()) {
            Some(p) => p,
            None => self.dir.clone(),
        };
        self.open_path(&target);
    }

    /// Open the folder the file sits in, not the file itself.
    pub fn reveal_item(&mut self, at: usize) {
        let target = match self.downloads.get(at).and_then(|d| d.path.clone()) {
            Some(p) => p.parent().unwrap_or(&self.dir).to_path_buf(),
            None => self.dir.clone(),
        };
        self.open_path(&target);
    }

    fn open_path(&mut self, path: &std::path::Path) {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        self.message = match std::process::Command::new(opener)
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => format!("opened {}", path.display()),
            Err(e) => format!("could not open {}: {e}", path.display()),
        };
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

    /// Sum of the running torrents' upload rates, in bytes/s.
    pub fn upload(&self) -> f64 {
        self.downloads
            .iter()
            .filter(|d| d.status == Status::Running)
            .filter_map(|d| bytes(&d.progress.upload))
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
