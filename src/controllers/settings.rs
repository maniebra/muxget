use std::path::PathBuf;

use crate::controllers::app::App;
use crate::models::state::State;
use crate::views::theme::Theme;

/// Preferences: theme and download directory. Backend options live in
/// `controllers::options`, which is a panel of its own.
impl App {
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.theme.save();
        self.message = format!("theme: {}", self.theme.name);
    }

    /// Nerd font glyphs for the status column; plain unicode otherwise.
    pub fn toggle_nerd(&mut self) {
        self.nerd = !self.nerd;
        self.save_state();
        self.message = if self.nerd {
            "nerd font icons on".into()
        } else {
            "nerd font icons off".into()
        };
    }

    /// Where new downloads are written. Running transfers keep their old dir.
    pub fn set_dir(&mut self, dir: &str) {
        let path = PathBuf::from(expand_home(dir.trim()));
        if !path.is_dir() {
            self.message = format!("not a directory: {}", path.display());
            return;
        }
        self.dir = path;
        self.save_state();
        self.message = format!("saving to {}", self.dir.display());
    }

    /// Persist the directory and queues. Called by every action that changes
    /// them, so there is no separate "save settings" step to forget.
    pub fn save_state(&self) {
        State::save(&self.dir, &self.queues, &self.downloads, self.nerd);
    }
}

/// Only `~`, which is what a typed path actually needs; the shell is not
/// involved here so nothing else would be expanded anyway.
fn expand_home(path: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) => {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}{rest}")
        }
        None => path.to_string(),
    }
}
