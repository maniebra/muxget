use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::controllers::downloads::Filter;
use crate::controllers::keys::Dialog;
use crate::controllers::options::Settings;
use crate::models::download::{Download, Status, Update};
use crate::models::queue::Queue;
use crate::models::rule::{self, Rule};
use crate::models::state::State;
use crate::views;
use crate::views::theme::Theme;

/// Shared state plus the event loop. The behaviour lives in sibling modules,
/// each owning one concern: `downloads`, `queues`, `settings`, `keys`.
pub struct App {
    pub dir: PathBuf,
    pub downloads: Vec<Download>,
    pub selected: usize,
    pub dialog: Option<Dialog>,
    /// Settings panel; owns the keyboard while open.
    pub settings: Option<Settings>,
    /// Half-typed key sequence, e.g. `g` waiting for `n`. Shows the menu.
    pub pending: Option<char>,
    pub message: String,
    pub filter: Filter,
    pub queues: Vec<Queue>,
    /// Index into `queues` — the queue being viewed; new downloads land here.
    pub current: usize,
    pub theme: Theme,
    /// Use nerd font glyphs for status icons.
    pub nerd: bool,
    /// Show a playlist's entries to pick from instead of queueing them all.
    pub confirm_playlist: bool,
    /// Routing rules, read once at startup.
    pub rules: Vec<Rule>,
    pub themes: Vec<Theme>,
    /// Aggregate bytes/s, newest last, for the sparkline.
    pub history: Vec<u64>,
    pub(crate) ticked: Instant,
    /// Last time queue schedules were checked against the clock.
    pub(crate) scheduled: Instant,
    pub(crate) next_id: usize,
    pub(crate) next_queue_id: usize,
    pub(crate) tx: Sender<Update>,
    rx: Receiver<Update>,
}

impl App {
    /// `dir` is the effective download directory: the `-d` flag if given,
    /// else the saved one, else the working directory (resolved in `main`).
    pub fn new(dir: PathBuf) -> Self {
        // Nothing is running yet, so any leftover credentials file is stale.
        crate::utils::clear_all_creds();
        let state = State::load();
        // A backend that outlived the last session would download behind this
        // one's back, into the same files, reporting to nobody.
        for d in &state.downloads {
            if let Some(pid) = d.pid {
                let name = crate::models::pick(&d.url).map_or("", |b| b.name());
                crate::utils::reap(pid, name);
            }
        }
        let mut app = App::with_queues(dir, state.queues_or_default());
        app.nerd = state.nerd;
        app.confirm_playlist = state.confirm_playlist;
        app.restore(&state.downloads);
        // Last, so it is the line the user is left looking at: nothing works
        // without a backend, and the failure would otherwise be one cryptic
        // "no such file" per download.
        let missing = crate::utils::missing_backends();
        if !missing.is_empty() {
            app.message = format!("not installed: {} — install to download", missing.join(", "));
        }
        app
    }

    /// Explicit queues instead of the saved ones — used by tests, which must
    /// not depend on whatever the last run happened to persist.
    pub fn with_queues(dir: PathBuf, queues: Vec<Queue>) -> Self {
        let (tx, rx) = channel();
        // Ids outlive positions now, so the next one is past the highest.
        let next_queue_id = queues.iter().map(|q| q.id + 1).max().unwrap_or(0);
        App {
            dir,
            downloads: Vec::new(),
            selected: 0,
            dialog: None,
            settings: None,
            pending: None,
            message: String::new(),
            filter: Filter::All,
            queues,
            current: 0,
            next_id: 0,
            next_queue_id,
            theme: Theme::saved().unwrap_or_default(),
            nerd: false,
            confirm_playlist: false,
            rules: rule::load(),
            themes: Theme::all(),
            history: Vec::new(),
            ticked: Instant::now(),
            scheduled: Instant::now(),
            tx,
            rx,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            self.drain();
            self.tick();
            terminal.draw(|f| views::ui::draw(f, &self))?;

            if !event::poll(Duration::from_millis(200))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if self.on_key(key.code) {
                        break;
                    }
                }
                Event::Mouse(mouse) => self.on_mouse(mouse, terminal.size()?),
                _ => {}
            }
        }

        // Children are ours; do not orphan them on exit.
        for d in &mut self.downloads {
            d.kill();
        }
        crate::utils::clear_all_creds();
        Ok(())
    }

    /// Apply everything the worker threads reported since the last frame.
    fn drain(&mut self) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Progress(id, p) => {
                    let total = crate::utils::parse::bytes(&p.total);
                    if let Some(d) = self.find(id) {
                        d.progress = p;
                    }
                    // Size rules can only be answered once a total is known.
                    if let Some(total) = total {
                        self.route_by_size(id, total);
                    }
                }
                Update::Located(id, path) => {
                    // yt-dlp may report a relative path.
                    let dir = self.dir.clone();
                    if let Some(d) = self.find(id) {
                        d.path = Some(dir.join(path));
                    }
                }
                Update::Discovered(queue, url, over) => self.enqueue(&url, queue, over),
                Update::Listed(listing) => self.listed(*listing),
                Update::Crawled(crawl, found) => self.crawled(crawl, found),
                Update::Notice(text) => self.message = text,
                Update::Finished(id, s) => {
                    if let Some(d) = self.find(id) {
                        d.pid = None;
                        if d.status == Status::Running {
                            d.status = s;
                        }
                    }
                    self.retry_failed(id);
                }
            }
        }
        // A finished download frees a slot for the next queued one.
        self.pump();
        self.save_state();
    }

    /// By stable id, never by index — rows move as the list is edited.
    fn find(&mut self, id: usize) -> Option<&mut Download> {
        self.downloads.iter_mut().find(|d| d.id == id)
    }

    /// Sample the aggregate speed for the sparkline, twice a second.
    fn tick(&mut self) {
        if self.ticked.elapsed() < Duration::from_millis(500) {
            return;
        }
        let elapsed = self.ticked.elapsed().as_secs_f64();
        self.ticked = Instant::now();
        self.charge_quotas(elapsed);
        // Windows have minute resolution.
        if self.scheduled.elapsed() >= Duration::from_secs(15) {
            self.scheduled = Instant::now();
            self.apply_schedules();
        }
        self.history.push(self.speed() as u64);
        if self.history.len() > 240 {
            self.history.remove(0);
        }
    }
}
