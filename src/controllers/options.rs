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
