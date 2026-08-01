use std::process::Stdio;

use crate::controllers::app::App;
use crate::controllers::keys::{Dialog, Field, Pick};
use crate::models::download::{Download, Overrides, Progress, Status, Update};
use crate::models::state::SavedDownload;
use crate::models::ytdlp::{DateRange, Listing};
use crate::models::{self, aria2, backend, pick, queue, ytdlp};
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

    /// Apply the first rule the url matches: its queue (created if it does not
    /// exist yet), its directory, its backend. Anything the rule leaves out,
    /// and anything typed into the add form, stays as it was.
    fn route(&mut self, url: &str, queue: usize, over: &mut Overrides) -> usize {
        // Size rules wait for a total; applying one here would route every
        // download to it, whatever it turns out to weigh.
        let rule = self
            .rules
            .iter()
            .find(|r| r.min_size.is_none() && r.matches(url))
            .cloned();
        let Some(rule) = rule else {
            return queue;
        };
        if over.dir.is_empty() {
            if let Some(dir) = &rule.directory {
                over.dir = crate::utils::expand_home(dir);
            }
        }
        if let Some(name) = &rule.backend {
            over.backend = name.clone();
        }
        match &rule.queue {
            Some(name) => self.queue_named(name),
            None => queue,
        }
    }

    /// The queue with this name, creating it if the rules mention one that
    /// does not exist yet — a category should not need setting up by hand.
    fn queue_named(&mut self, name: &str) -> usize {
        if let Some(q) = self.queues.iter().find(|q| q.name == name) {
            return q.id;
        }
        let id = self.next_queue_id;
        self.next_queue_id += 1;
        self.queues.push(queue::Queue::new(id, name, 3));
        id
    }

    /// Move a download to the queue of the first size rule it now satisfies.
    pub fn route_by_size(&mut self, id: usize, total: f64) {
        let Some(at) = self.downloads.iter().position(|d| d.id == id) else { return };
        let url = self.downloads[at].url.clone();
        let rule = self
            .rules
            .iter()
            .find(|r| r.min_size.is_some() && r.matches(&url) && r.wants_size(total))
            .cloned();
        let Some(name) = rule.and_then(|r| r.queue) else { return };
        let to = self.queue_named(&name);
        if self.downloads[at].queue != to {
            self.downloads[at].queue = to;
            self.message = format!("{url} routed to {name}");
            self.save_state();
        }
    }

    /// Add one url to `queue`. It starts as soon as that queue has a slot.
    pub(crate) fn enqueue(&mut self, url: &str, queue: usize, mut over: Overrides) {
        let queue = self.route(url, queue, &mut over);
        let Some(backend) = backend_for(url, &over) else {
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
            pid: None,
            tries: 0,
        });
        self.message = format!("queued {url}");
        self.save_state();
        self.pump();
    }

    /// Read the clipboard and offer whatever urls are in it. Nothing is
    /// queued until the preview is accepted — a clipboard is full of things
    /// that are not downloads.
    pub fn paste(&mut self) {
        let Some(text) = crate::utils::clipboard() else {
            self.message = "no clipboard tool found (wl-paste, xclip, xsel, pbpaste)".into();
            return;
        };
        self.preview_paste(&text);
    }

    /// The clipboard's text, as a preview. Split out so it can be driven
    /// without a clipboard.
    pub fn preview_paste(&mut self, text: &str) {
        let urls = crate::utils::urls_in(text);
        if urls.is_empty() {
            self.message = "no urls in the clipboard".into();
            return;
        }
        self.message = format!("{} urls in the clipboard", urls.len());
        // All picked: the usual paste is a list someone meant to download.
        let picked = (0..urls.len()).collect();
        self.dialog = Some(Dialog::Paste(urls, picked, 0));
    }

    /// Queue the urls that survived the preview, each routed as if typed.
    pub fn add_pasted(&mut self, urls: &[String], picked: &[usize]) {
        for url in picked.iter().filter_map(|i| urls.get(*i)) {
            self.add(url);
        }
        self.message = format!("added {} of {} pasted urls", picked.len(), urls.len());
    }

    /// List a playlist's entries off-thread; each one arrives as `Discovered`,
    /// or the whole list as `Listed` when the user wants to pick from it.
    fn expand_playlist(&mut self, url: &str, over: Overrides) {
        let queue = self.queue().id;
        let confirm = self.confirm_playlist;
        self.list_playlist(
            Listing { url: url.to_string(), queue, over, ..Default::default() },
            confirm,
        );
    }

    /// Run the listing pass. Called again, with a date range this time, when
    /// the picker asks for one.
    pub(crate) fn list_playlist(&mut self, listing: Listing, confirm: bool) {
        let tx = self.tx.clone();
        let Listing { url, queue, over, dates, .. } = listing;
        self.message = match dates.is_empty() {
            true => format!("expanding playlist {url}…"),
            // Every entry has to be opened for its date, so say so.
            false => format!("reading upload dates for {url} — this is slower…"),
        };

        std::thread::spawn(move || {
            let mut child = match ytdlp::list_command(&url, &dates)
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

            let mut entries = Vec::new();
            for_each_line(stdout, |line| {
                let Some(entry) = ytdlp::entry(line) else { return };
                // Confirming holds them back, so the picker gets the list in
                // one piece; otherwise each one is queued as it arrives.
                if !confirm {
                    let _ = tx.send(Update::Discovered(queue, entry.0.clone(), over.clone()));
                }
                entries.push(entry);
            });
            let _ = child.wait();
            if entries.is_empty() {
                let _ = tx.send(Update::Notice(format!("no playlist entries found in {url}")));
                return;
            }
            let _ = match confirm {
                true => tx.send(Update::Listed(Box::new(Listing {
                    url,
                    queue,
                    over,
                    dates,
                    entries,
                }))),
                false => tx.send(Update::Notice(format!(
                    "queued {} entries from the playlist",
                    entries.len()
                ))),
            };
        });
    }

    /// A playlist came back listed rather than queued: show it to pick from.
    pub fn listed(&mut self, listing: Listing) {
        if listing.entries.is_empty() {
            self.message = format!("nothing in {} matched that date range", listing.url);
            return;
        }
        self.message = format!("{} entries — pick which to download", listing.entries.len());
        // Everything picked to start with: the common case is "all but a
        // few", and `a` clears the lot for the opposite one.
        self.dialog = Some(Dialog::Playlist(Box::new(Pick {
            picked: (0..listing.entries.len()).collect(),
            listing,
            at: 0,
            words: String::new(),
            editing: None,
        })));
    }

    /// One of the picker's fields was typed into and accepted. Returns true
    /// when the picker is gone — a date range is answered by yt-dlp, not by
    /// the list already on screen, so it has to be listed again.
    pub fn typed_into_picker(&mut self, pick: &mut Pick, field: Field, buf: &str) -> bool {
        match field {
            Field::Dir => pick.listing.over.dir = crate::utils::expand_home(buf.trim()),
            Field::Words => {
                pick.words = buf.trim().to_lowercase();
                // The filter is the selection: what it hides is not queued,
                // and what it reveals is picked and waiting.
                pick.picked = pick.shown();
                pick.at = 0;
            }
            Field::Dates => {
                let mut listing = pick.listing.clone();
                listing.dates = DateRange::parse(buf);
                self.list_playlist(listing, true);
                return true;
            }
        }
        false
    }

    /// Queue the picked entries, each with the settings shown in the picker.
    pub fn add_listed(&mut self, pick: &Pick) {
        let mut picked: Vec<usize> = pick.picked.clone();
        picked.sort_unstable();
        for (url, _) in picked.iter().filter_map(|i| pick.listing.entries.get(*i)) {
            self.enqueue(url, pick.listing.queue, pick.listing.over.clone());
        }
        self.message =
            format!("queued {} of {} entries", picked.len(), pick.listing.entries.len());
    }

    /// Rebuild last session's list. Ids are reassigned — the saved ones meant
    /// nothing to the processes that are now gone. Unfinished rows come back
    /// queued and resume from their partial files.
    pub fn restore(&mut self, saved: &[SavedDownload]) {
        let known: Vec<usize> = self.queues.iter().map(|q| q.id).collect();
        for s in saved {
            let Some(backend) = backend_for(&s.url, &s.over) else { continue };
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
                path: s.path.clone(),
                pid: None,
                tries: s.tries,
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
        let Some(backend) = backend_for(&url, &over) else { return };
        let name = backend.name();
        match backend::run(backend, &url, &self.dir, &over, id, self.tx.clone()) {
            Ok(pid) => {
                let d = &mut self.downloads[at];
                d.pid = Some(pid);
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
            // resuming can put a queue one over its limit until
            // something finishes; give resume its own wait state if that bites.
            Status::Paused => {
                self.resume_row(at);
                self.message = format!("resumed {}", self.downloads[at].url);
            }
            _ => {}
        }
    }

    /// Resume a paused row. One paused across a restart has no process left to
    /// continue, so it goes back in the queue and starts from its partial file
    /// as soon as its queue has a slot.
    pub(crate) fn resume_row(&mut self, at: usize) {
        let Some(d) = self.downloads.get_mut(at) else { return };
        match d.pid {
            Some(_) => d.resume(),
            None => {
                d.status = Status::Queued;
                self.pump();
            }
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

    /// Sum of the running downloads' speeds in one queue, in bytes/s.
    pub fn speed_in(&self, queue: usize) -> f64 {
        self.downloads
            .iter()
            .filter(|d| d.queue == queue && d.status == Status::Running)
            .filter_map(|d| bytes(&d.progress.speed))
            .sum()
    }

    /// A download that failed goes back in the queue while its queue's retry
    /// limit allows it; past that the failure sticks.
    pub fn retry_failed(&mut self, id: usize) {
        let Some(at) = self.downloads.iter().position(|d| d.id == id) else { return };
        let queue = self.downloads[at].queue;
        let limit = self.queues.iter().find(|q| q.id == queue).map_or(0, |q| q.retry);
        let d = &mut self.downloads[at];
        if !matches!(d.status, Status::Failed(_)) || d.tries >= limit {
            return;
        }
        d.tries += 1;
        d.status = Status::Queued;
        self.message = format!("retry {}/{limit} for {}", self.downloads[at].tries, self.downloads[at].url);
        self.pump();
    }

    /// Requeue a failed or cancelled download by hand, ignoring the queue's
    /// retry limit — the user asking is the decision. Tries reset so the
    /// automatic retries get their full budget again.
    pub fn retry(&mut self, at: usize) {
        let Some(d) = self.downloads.get_mut(at) else { return };
        if !matches!(d.status, Status::Failed(_) | Status::Cancelled) {
            return;
        }
        d.tries = 0;
        d.status = Status::Queued;
        self.message = format!("retrying {}", self.downloads[at].url);
        self.pump();
        self.save_state();
    }

    /// Sum of the running torrents' upload rates, in bytes/s.
    pub fn upload(&self) -> f64 {
        self.downloads
            .iter()
            .filter(|d| d.status == Status::Running)
            .filter_map(|d| bytes(&d.progress.upload))
            .sum()
    }

    /// Move the selected row `delta` places within what is on screen. Order is
    /// priority: `next_queued` starts the first waiting row it finds.
    pub fn move_download(&mut self, delta: isize) {
        let rows = self.visible();
        let Some(at) = rows.iter().position(|i| *i == self.selected) else { return };
        let Some(to) = at.checked_add_signed(delta).filter(|to| *to < rows.len()) else {
            return;
        };
        self.downloads.swap(rows[at], rows[to]);
        self.selected = rows[to];
        self.save_state();
        self.pump();
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

/// The backend a download runs on: the one a rule named, else the first that
/// claims the url.
fn backend_for(url: &str, over: &Overrides) -> Option<Box<dyn backend::Backend>> {
    match over.backend.is_empty() {
        true => pick(url),
        false => models::named(&over.backend).or_else(|| pick(url)),
    }
}
