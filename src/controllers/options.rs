use crossterm::event::KeyCode;

use crate::models::option::{specs, Kind, OptSpec};
use crate::utils::args;

/// The options panel for one backend: a form over `models::option::specs`,
/// backed by the same `<backend>.args` file the spawner reads.
pub struct Options {
    pub backend: &'static str,
    pub cursor: usize,
    /// Every flag in the file, known or not, in file order.
    pub pairs: Vec<(String, String)>,
    /// Text being typed for the selected value option.
    pub editing: Option<String>,
}

impl Options {
    pub fn open(backend: &'static str) -> Options {
        Options {
            backend,
            cursor: 0,
            pairs: args::to_pairs(&args::load(backend)),
            editing: None,
        }
    }

    pub fn specs(&self) -> &'static [OptSpec] {
        specs(self.backend)
    }

    /// Current value: `None` when unset, `Some("")` for an enabled bare flag.
    pub fn value(&self, flag: &str) -> Option<&str> {
        let (name, _) = split(flag);
        self.pairs
            .iter()
            .find(|(f, _)| f == name)
            .map(|(_, v)| v.as_str())
    }

    /// A spec may carry its value (`--seed-time=0`) while the file stores it
    /// split, so a match is on the name and — when the spec pins one — the
    /// value too.
    pub fn is_set(&self, flag: &str) -> bool {
        let (name, want) = split(flag);
        self.pairs
            .iter()
            .any(|(f, v)| f == name && (want.is_empty() || v == want))
    }

    fn set(&mut self, flag: &str, value: &str) {
        let (name, pinned) = split(flag);
        let value = if value.is_empty() { pinned } else { value };
        match self.pairs.iter_mut().find(|(f, _)| f == name) {
            Some(pair) => pair.1 = value.to_string(),
            None => self.pairs.push((name.to_string(), value.to_string())),
        }
    }

    pub fn unset(&mut self, flag: &str) {
        let (name, _) = split(flag);
        self.pairs.retain(|(f, _)| f != name);
    }

    pub fn toggle(&mut self, flag: &str) {
        if self.is_set(flag) {
            self.unset(flag);
        } else {
            self.set(flag, "");
        }
    }

    /// Flags with no spec — shown read-only so the panel never eats them.
    pub fn unknown(&self) -> Vec<&(String, String)> {
        self.pairs
            .iter()
            .filter(|(f, _)| !self.specs().iter().any(|s| split(s.flag).0 == f))
            .collect()
    }

    /// Handle a keypress. Returns true when the panel should close and save.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        let Some(spec) = self.specs().get(self.cursor) else {
            return true;
        };
        let (flag, kind_is_value) = (spec.flag, spec.kind == Kind::Value);

        // Editing a value takes the keyboard until Enter or Esc.
        if let Some(mut buf) = self.editing.take() {
            match key {
                KeyCode::Enter => {
                    if buf.trim().is_empty() {
                        self.unset(flag);
                    } else {
                        self.set(flag, buf.trim());
                    }
                }
                KeyCode::Esc => {}
                KeyCode::Backspace => {
                    buf.pop();
                    self.editing = Some(buf);
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.editing = Some(buf);
                }
                _ => self.editing = Some(buf),
            }
            return false;
        }

        match key {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(self.specs().len() - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') => self.cursor = 0,
            KeyCode::Char('G') => self.cursor = self.specs().len() - 1,
            KeyCode::Char('x') | KeyCode::Delete => self.unset(flag),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if kind_is_value {
                    self.editing = Some(self.value(flag).unwrap_or_default().to_string());
                } else {
                    self.toggle(flag);
                }
            }
            _ => {}
        }
        false
    }

    pub fn save(&self) -> std::io::Result<()> {
        args::save(self.backend, &args::render(&self.pairs))
    }
}

/// A spec flag as (name, pinned value); the value is empty for a bare flag.
fn split(flag: &str) -> (&str, &str) {
    match flag.split_once('=') {
        Some((name, value)) => (name, value),
        None => (flag, ""),
    }
}

/// The settings panel: a tab bar over the things that used to be scattered
/// across single keys. `Options` above is what the backends tab shows.
pub struct Settings {
    pub tab: usize,
    pub cursor: usize,
    pub options: Options,
}

pub const TABS: [&str; 3] = ["general", "backends", "categories"];
pub const GENERAL: [&str; 4] =
    ["theme", "download directory", "nerd font icons", "confirm before dl playlist"];

/// What the panel asks the app to do; everything else it handles itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    None,
    /// Save the backend form and close.
    Close,
    NextTheme,
    PrevTheme,
    EditDir,
    ToggleNerd,
    ToggleConfirmPlaylist,
}

impl Settings {
    pub fn open(tab: usize, backend: &'static str) -> Settings {
        Settings { tab, cursor: 0, options: Options::open(backend) }
    }

    pub fn rows(&self) -> usize {
        match self.tab {
            0 => GENERAL.len(),
            1 => self.options.specs().len(),
            _ => 0,
        }
    }

    /// Switching backend saves the form first; the file is the state.
    pub fn set_backend(&mut self, backend: &'static str) -> std::io::Result<()> {
        if self.options.backend == backend {
            return Ok(());
        }
        let saved = self.options.save();
        self.options = Options::open(backend);
        self.cursor = 0;
        saved
    }

    pub fn on_key(&mut self, key: KeyCode) -> Action {
        // The backend form owns the keyboard while a value is being typed.
        if self.tab == 1 && self.options.editing.is_some() {
            self.options.cursor = self.cursor;
            self.options.on_key(key);
            return Action::None;
        }
        match key {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.go(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.go(-1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = (self.cursor + 1).min(self.rows().saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            key => return self.act(key),
        }
        Action::None
    }

    fn go(&mut self, delta: isize) {
        self.tab = (self.tab as isize + delta).rem_euclid(TABS.len() as isize) as usize;
        self.cursor = 0;
    }

    fn act(&mut self, key: KeyCode) -> Action {
        match (self.tab, self.cursor, key) {
            (0, 0, KeyCode::Enter) | (0, 0, KeyCode::Char(' ')) => Action::NextTheme,
            (0, 0, KeyCode::Char('T')) => Action::PrevTheme,
            (0, 1, KeyCode::Enter) => Action::EditDir,
            (0, 2, KeyCode::Enter) | (0, 2, KeyCode::Char(' ')) => Action::ToggleNerd,
            (0, 3, KeyCode::Enter) | (0, 3, KeyCode::Char(' ')) => Action::ToggleConfirmPlaylist,
            // One backend's form at a time; `b` walks to the next.
            (1, _, KeyCode::Char('b')) => {
                let names: Vec<&'static str> =
                    crate::models::backends().iter().map(|b| b.name()).collect();
                let at = names.iter().position(|n| *n == self.options.backend).unwrap_or(0);
                let next = names[(at + 1) % names.len()];
                let _ = self.set_backend(next);
                Action::None
            }
            (1, _, key) => {
                self.options.cursor = self.cursor;
                self.options.on_key(key);
                Action::None
            }
            _ => Action::None,
        }
    }
}
