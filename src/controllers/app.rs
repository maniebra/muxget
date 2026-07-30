use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::models::download::{Download, Status, Update};
use crate::models::{backend, pick};
use crate::views;
use crate::views::theme::Theme;

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
            Filter::Active => matches!(status, Status::Running | Status::Queued),
            Filter::Done => *status == Status::Done,
            Filter::Failed => matches!(status, Status::Failed(_) | Status::Cancelled),
        }
    }
}

pub struct App {
    pub dir: PathBuf,
    pub downloads: Vec<Download>,
    pub selected: usize,
    pub dialog: Option<Dialog>,
    pub message: String,
    pub filter: Filter,
    /// How many downloads may run at once; the rest wait as Queued.
    pub max_active: usize,
    pub theme: Theme,
    pub themes: Vec<Theme>,
    /// Aggregate bytes/s, newest last, for the sparkline.
    pub history: Vec<u64>,
    ticked: Instant,
    next_id: usize,
    tx: Sender<Update>,
    rx: Receiver<Update>,
}

/// Modal state. `Some` means the popover is up and owns the keyboard.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    /// New url being typed.
    Add(String),
    /// Editing the url of the download at this index; edit restarts it.
    Edit(usize, String),
    /// Confirming removal of the download at this index.
    Delete(usize),
}

impl App {
    pub fn new(dir: PathBuf) -> Self {
        let (tx, rx) = channel();
        App {
            dir,
            downloads: Vec::new(),
            selected: 0,
            dialog: None,
            message: String::new(),
            filter: Filter::All,
            max_active: 3,
            next_id: 0,
            theme: Theme::saved().unwrap_or_default(),
            themes: Theme::all(),
            history: Vec::new(),
            ticked: Instant::now(),
            tx,
            rx,
        }
    }

    /// Enqueue a url. It starts as soon as a slot is free.
    pub fn add(&mut self, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        let Some(backend) = pick(url) else {
            self.message = format!("no backend accepts {url}");
            return;
        };
        let id = self.next_id;
        self.next_id += 1;
        self.downloads.push(Download {
            id,
            url: url.to_string(),
            backend: backend.name(),
            status: Status::Queued,
            progress: Default::default(),
            child: None,
        });
        self.message = format!("queued {url}");
        self.pump();
    }

    pub fn active(&self) -> usize {
        self.downloads.iter().filter(|d| d.status == Status::Running).count()
    }

    /// Index of the download that should start next, oldest first.
    pub fn next_queued(&self) -> Option<usize> {
        self.downloads.iter().position(|d| d.status == Status::Queued)
    }

    /// Start queued downloads until the concurrency limit is reached.
    pub fn pump(&mut self) {
        while self.active() < self.max_active {
            let Some(at) = self.next_queued() else { break };
            self.start(at);
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

    /// Change how many downloads may run at once; frees or fills slots at once.
    pub fn set_max_active(&mut self, n: usize) {
        self.max_active = n.clamp(1, 16);
        self.message = format!("{} concurrent", self.max_active);
        self.pump();
    }

    /// Stop the download but keep the row.
    pub fn cancel(&mut self, at: usize) {
        if let Some(d) = self.downloads.get_mut(at) {
            if matches!(d.status, Status::Running | Status::Queued) {
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

    fn clamp_selection(&mut self) {
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

    fn find(&mut self, id: usize) -> Option<&mut Download> {
        self.downloads.iter_mut().find(|d| d.id == id)
    }

    fn drain(&mut self) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Progress(id, p) => {
                    if let Some(d) = self.find(id) {
                        d.progress = p;
                    }
                }
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
    }

    /// Indices of the downloads the current filter shows, in list order.
    pub fn visible(&self) -> Vec<usize> {
        self.downloads
            .iter()
            .enumerate()
            .filter(|(_, d)| self.filter.matches(&d.status))
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

    /// Sum of the running downloads' reported speeds, in bytes/s.
    pub fn speed(&self) -> f64 {
        self.downloads
            .iter()
            .filter(|d| d.status == Status::Running)
            .filter_map(|d| crate::utils::parse::bytes(&d.progress.speed))
            .sum()
    }

    fn tick(&mut self) {
        if self.ticked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.ticked = Instant::now();
        self.history.push(self.speed() as u64);
        if self.history.len() > 240 {
            self.history.remove(0);
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
        Ok(())
    }

    /// Handle one keypress. Returns true when the app should quit.
    /// An open dialog owns the keyboard until it closes.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        match self.dialog.take() {
            Some(Dialog::Delete(at)) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::Delete(at)),
            },
            Some(Dialog::Add(buf)) => {
                if let Some(text) = self.type_into(buf, key, Dialog::Add) {
                    self.add(&text);
                }
            }
            Some(Dialog::Edit(at, buf)) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::Edit(at, b)) {
                    self.edit(at, &text);
                }
            }
            None => match key {
                KeyCode::Char('q') => return true,
                KeyCode::Char('a') => self.dialog = Some(Dialog::Add(String::new())),
                KeyCode::Char('e') => {
                    if let Some(d) = self.downloads.get(self.selected) {
                        self.dialog = Some(Dialog::Edit(self.selected, d.url.clone()));
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if self.downloads.get(self.selected).is_some() {
                        self.dialog = Some(Dialog::Delete(self.selected));
                    }
                }
                KeyCode::Char('x') => self.cancel(self.selected),
                KeyCode::Char('+') | KeyCode::Char('=') => self.set_max_active(self.max_active + 1),
                KeyCode::Char('-') => self.set_max_active(self.max_active.saturating_sub(1)),
                KeyCode::Char('t') => {
                    self.theme = self.theme.next(&self.themes);
                    self.theme.save();
                    self.message = format!("theme: {}", self.theme.name);
                }
                KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
                KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
                KeyCode::Tab | KeyCode::Char('f') => {
                    let i = Filter::ALL.iter().position(|f| *f == self.filter).unwrap_or(0);
                    self.set_filter(Filter::ALL[(i + 1) % Filter::ALL.len()]);
                }
                _ => {}
            },
        }
        false
    }

    /// Text-field keys shared by the add and edit dialogs. Returns the text on
    /// Enter; otherwise reopens the dialog through `reopen` with the new buffer.
    fn type_into(
        &mut self,
        mut buf: String,
        key: KeyCode,
        reopen: impl Fn(String) -> Dialog,
    ) -> Option<String> {
        match key {
            KeyCode::Enter => return Some(buf),
            KeyCode::Esc => {}
            KeyCode::Backspace => {
                buf.pop();
                self.dialog = Some(reopen(buf));
            }
            KeyCode::Char(c) => {
                buf.push(c);
                self.dialog = Some(reopen(buf));
            }
            _ => self.dialog = Some(reopen(buf)),
        }
        None
    }
}
