use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::controllers::downloads::Filter;
use crate::controllers::keys::Dialog;
use crate::controllers::options::Options;
use crate::models::download::{Download, Status, Update};
use crate::models::queue::Queue;
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
    /// Backend options panel; owns the keyboard while open.
    pub options: Option<Options>,
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
        let mut app = App::with_queues(dir, state.queues_or_default());
        app.nerd = state.nerd;
        app.restore(&state.downloads);
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
            options: None,
            pending: None,
            message: String::new(),
            filter: Filter::All,
            queues,
            current: 0,
            next_id: 0,
            next_queue_id,
            theme: Theme::saved().unwrap_or_default(),
            nerd: false,
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
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if self.on_key(key.code) {
                break;
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
                    if let Some(d) = self.find(id) {
                        d.progress = p;
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
                Update::Notice(text) => self.message = text,
                Update::Finished(id, s) => {
                    if let Some(d) = self.find(id) {
                        d.child = None;
                        if d.status == Status::Running {
                            d.status = s;
                        }
                    }
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
        self.ticked = Instant::now();
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
