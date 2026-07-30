use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::models::download::{Download, Status, Update};
use crate::models::queue::{self, Queue};
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
    /// Half-typed key sequence, e.g. `g` waiting for `n`. Shows the menu.
    pub pending: Option<char>,
    pub message: String,
    pub filter: Filter,
    pub queues: Vec<Queue>,
    /// Index into `queues` — the queue being viewed; new downloads land here.
    pub current: usize,
    pub theme: Theme,
    pub themes: Vec<Theme>,
    /// Aggregate bytes/s, newest last, for the sparkline.
    pub history: Vec<u64>,
    ticked: Instant,
    next_id: usize,
    next_queue_id: usize,
    tx: Sender<Update>,
    rx: Receiver<Update>,
}

/// Only `~`, which is what a typed path actually needs; the shell is not
/// involved here so nothing else would be expanded anyway.
fn shellexpand(path: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) => {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}{rest}")
        }
        None => path.to_string(),
    }
}

fn char_of(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

fn key_char(key: KeyCode) -> char {
    char_of(key).unwrap_or('\0')
}

/// One entry of a which-key style menu: press the prefix, then this key.
pub struct MenuItem {
    pub key: char,
    pub label: &'static str,
}

/// Queue commands, reached with the `g` prefix (`gn`, `gr`, …); `q` stays
/// quit, and `g` is free because there is nothing to jump to. The menu
/// popover renders this table, so adding an entry documents itself.
pub const QUEUE_MENU: &[MenuItem] = &[
    MenuItem { key: 'n', label: "new queue" },
    MenuItem { key: 'r', label: "rename queue" },
    MenuItem { key: 'd', label: "delete queue" },
    MenuItem { key: 'j', label: "next queue" },
    MenuItem { key: 'k', label: "previous queue" },
    MenuItem { key: '+', label: "one more slot" },
    MenuItem { key: '-', label: "one less slot" },
];

/// Settings commands, reached with the `s` prefix.
pub const SETTINGS_MENU: &[MenuItem] = &[
    MenuItem { key: 't', label: "next theme" },
    MenuItem { key: 'T', label: "previous theme" },
    MenuItem { key: 'd', label: "download directory" },
];

pub fn menu_for(prefix: char) -> Option<(&'static str, &'static [MenuItem])> {
    match prefix {
        'g' => Some(("queue", QUEUE_MENU)),
        's' => Some(("settings", SETTINGS_MENU)),
        _ => None,
    }
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
    /// Name for a new queue.
    QueueNew(String),
    /// Renaming the queue at this index in `queues`.
    QueueRename(usize, String),
    /// Confirming removal of the queue at this index in `queues`.
    QueueDelete(usize),
    /// New download directory being typed.
    SetDir(String),
}

impl App {
    pub fn new(dir: PathBuf) -> Self {
        let (tx, rx) = channel();
        App {
            dir,
            downloads: Vec::new(),
            selected: 0,
            dialog: None,
            pending: None,
            message: String::new(),
            filter: Filter::All,
            queues: vec![Queue::new(queue::DEFAULT, "default", 3)],
            current: 0,
            next_id: 0,
            next_queue_id: 1,
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
            queue: self.queue().id,
            url: url.to_string(),
            backend: backend.name(),
            status: Status::Queued,
            progress: Default::default(),
            child: None,
        });
        self.message = format!("queued {url}");
        self.pump();
    }

    /// The queue currently being viewed. Always valid — `current` is clamped
    /// on every change and the default queue can never be removed.
    pub fn queue(&self) -> &Queue {
        &self.queues[self.current.min(self.queues.len() - 1)]
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

    /// Fill every queue's free slots. Queues run independently of each other.
    pub fn pump(&mut self) {
        for i in 0..self.queues.len() {
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

    /// Change how many downloads the current queue may run at once.
    pub fn set_max_active(&mut self, n: usize) {
        let i = self.current;
        self.queues[i].max_active = n.clamp(1, 16);
        self.message = format!("{}: {} concurrent", self.queues[i].name, self.queues[i].max_active);
        self.pump();
    }

    /// Create a queue and switch to it. Blank or duplicate names are refused.
    pub fn add_queue(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if self.queues.iter().any(|q| q.name == name) {
            self.message = format!("queue {name} already exists");
            return;
        }
        let id = self.next_queue_id;
        self.next_queue_id += 1;
        self.queues.push(Queue::new(id, name, 3));
        self.current = self.queues.len() - 1;
        self.clamp_selection();
        self.message = format!("queue {name} created");
    }

    pub fn rename_queue(&mut self, at: usize, name: &str) {
        let name = name.trim();
        if name.is_empty() || at >= self.queues.len() {
            return;
        }
        if self.queues.iter().enumerate().any(|(i, q)| i != at && q.name == name) {
            self.message = format!("queue {name} already exists");
            return;
        }
        self.queues[at].name = name.to_string();
    }

    /// Remove a queue; its downloads move to the default queue rather than
    /// being silently killed. The default queue itself cannot be removed.
    pub fn delete_queue(&mut self, at: usize) {
        let Some(q) = self.queues.get(at) else { return };
        if q.id == queue::DEFAULT {
            self.message = "the default queue cannot be deleted".into();
            return;
        }
        let (id, name) = (q.id, q.name.clone());
        for d in self.downloads.iter_mut().filter(|d| d.queue == id) {
            d.queue = queue::DEFAULT;
        }
        self.queues.remove(at);
        self.current = self.current.min(self.queues.len() - 1);
        self.clamp_selection();
        self.message = format!("queue {name} deleted, downloads moved to default");
        self.pump();
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.theme.save();
        self.message = format!("theme: {}", self.theme.name);
    }

    /// Where new downloads are written. Running transfers keep their old dir.
    pub fn set_dir(&mut self, dir: &str) {
        let path = PathBuf::from(shellexpand(dir.trim()));
        if !path.is_dir() {
            self.message = format!("not a directory: {}", path.display());
            return;
        }
        self.dir = path;
        self.message = format!("saving to {}", self.dir.display());
    }

    /// Switch the viewed queue by `delta` places.
    pub fn cycle_queue(&mut self, delta: isize) {
        let n = self.queues.len() as isize;
        self.current = (self.current as isize + delta).rem_euclid(n) as usize;
        self.clamp_selection();
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
        // A half-typed sequence swallows the next key, vim style.
        if let Some(prefix) = self.pending.take() {
            return self.on_sequence(prefix, key);
        }

        match self.dialog.take() {
            Some(Dialog::Delete(at)) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::Delete(at)),
            },
            Some(Dialog::QueueDelete(at)) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete_queue(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::QueueDelete(at)),
            },
            Some(Dialog::SetDir(buf)) => {
                if let Some(text) = self.type_into(buf, key, Dialog::SetDir) {
                    self.set_dir(&text);
                }
            }
            Some(Dialog::QueueNew(buf)) => {
                if let Some(text) = self.type_into(buf, key, Dialog::QueueNew) {
                    self.add_queue(&text);
                }
            }
            Some(Dialog::QueueRename(at, buf)) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::QueueRename(at, b)) {
                    self.rename_queue(at, &text);
                }
            }
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
                KeyCode::Char('g') | KeyCode::Char('s') | KeyCode::Char('Z') => {
                    self.pending = Some(key_char(key))
                }
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
                KeyCode::Char(']') | KeyCode::Right => self.cycle_queue(1),
                KeyCode::Char('[') | KeyCode::Left => self.cycle_queue(-1),
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

    /// Second key of a sequence. Anything unlisted quietly cancels, as in vim.
    fn on_sequence(&mut self, prefix: char, key: KeyCode) -> bool {
        let Some(c) = char_of(key) else { return false };
        match (prefix, c) {
            ('Z', 'Z') | ('Z', 'Q') => return true,
            ('s', 't') => self.set_theme(self.theme.next(&self.themes)),
            ('s', 'T') => self.set_theme(self.theme.prev(&self.themes)),
            ('s', 'd') => {
                self.dialog = Some(Dialog::SetDir(self.dir.display().to_string()));
            }
            ('g', 'n') => self.dialog = Some(Dialog::QueueNew(String::new())),
            ('g', 'r') => {
                self.dialog = Some(Dialog::QueueRename(self.current, self.queue().name.clone()))
            }
            ('g', 'd') => self.dialog = Some(Dialog::QueueDelete(self.current)),
            ('g', 'j') | ('g', 'l') => self.cycle_queue(1),
            ('g', 'k') | ('g', 'h') => self.cycle_queue(-1),
            ('g', '+') | ('g', '=') => self.set_max_active(self.queue().max_active + 1),
            ('g', '-') => self.set_max_active(self.queue().max_active - 1),
            _ => {}
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
