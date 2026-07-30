use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
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
            Filter::Active => *status == Status::Running,
            Filter::Done => *status == Status::Done,
            Filter::Failed => matches!(status, Status::Failed(_) | Status::Cancelled),
        }
    }
}

pub struct App {
    pub dir: PathBuf,
    pub downloads: Vec<Download>,
    pub selected: usize,
    pub input: Option<String>,
    pub message: String,
    pub filter: Filter,
    pub theme: Theme,
    pub themes: Vec<Theme>,
    /// Aggregate bytes/s, newest last, for the sparkline.
    pub history: Vec<u64>,
    ticked: Instant,
    children: Vec<Option<Arc<Mutex<Child>>>>,
    tx: Sender<Update>,
    rx: Receiver<Update>,
}

impl App {
    pub fn new(dir: PathBuf) -> Self {
        let (tx, rx) = channel();
        App {
            dir,
            downloads: Vec::new(),
            selected: 0,
            input: None,
            message: String::new(),
            filter: Filter::All,
            theme: Theme::saved().unwrap_or_default(),
            themes: Theme::all(),
            history: Vec::new(),
            ticked: Instant::now(),
            children: Vec::new(),
            tx,
            rx,
        }
    }

    pub fn add(&mut self, url: &str) {
        let url = url.trim();
        if url.is_empty() {
            return;
        }
        let Some(backend) = pick(url) else {
            self.message = format!("no backend accepts {url}");
            return;
        };
        let id = self.downloads.len();
        let name = backend.name();
        match backend::run(backend, url, &self.dir, id, self.tx.clone()) {
            Ok(child) => {
                self.downloads.push(Download {
                    url: url.to_string(),
                    backend: name,
                    status: Status::Running,
                    progress: Default::default(),
                });
                self.children.push(Some(child));
                self.message = format!("started {url}");
            }
            Err(e) => self.message = format!("{name} failed to start: {e}"),
        }
    }

    pub fn cancel(&mut self, id: usize) {
        if let Some(Some(child)) = self.children.get(id) {
            let _ = child.lock().unwrap().kill();
            self.downloads[id].status = Status::Cancelled;
        }
    }

    fn drain(&mut self) {
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Progress(id, p) => self.downloads[id].progress = p,
                Update::Finished(id, s) => {
                    self.children[id] = None;
                    if self.downloads[id].status == Status::Running {
                        self.downloads[id].status = s;
                    }
                }
            }
        }
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

            match self.input.take() {
                Some(mut buf) => match key.code {
                    KeyCode::Enter => self.add(&buf),
                    KeyCode::Esc => {}
                    KeyCode::Backspace => {
                        buf.pop();
                        self.input = Some(buf);
                    }
                    KeyCode::Char(c) => {
                        buf.push(c);
                        self.input = Some(buf);
                    }
                    _ => self.input = Some(buf),
                },
                None => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('a') => self.input = Some(String::new()),
                    KeyCode::Char('x') => self.cancel(self.selected),
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
        }

        // Children are ours; do not orphan them on exit.
        for child in self.children.iter().flatten() {
            let _ = child.lock().unwrap().kill();
        }
        Ok(())
    }
}
