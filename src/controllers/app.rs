use std::path::PathBuf;
use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::models::download::{Download, Status, Update};
use crate::models::{backend, pick};
use crate::views;

pub struct App {
    pub dir: PathBuf,
    pub downloads: Vec<Download>,
    pub selected: usize,
    pub input: Option<String>,
    pub message: String,
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
                self.message = format!("{name}: {url}");
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

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            self.drain();
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
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.selected = (self.selected + 1).min(self.downloads.len().saturating_sub(1))
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
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
