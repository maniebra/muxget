use std::path::PathBuf;

use crate::controllers::app::App;
use crate::models::channel;
use crate::models::download::Overrides;
use crate::models::rule::{self, Rule};
use crate::models::ytdlp::{DateRange, Listing};
use crate::models::state::State;
use crate::utils::expand_home;
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

    /// Ask which entries to download when a playlist url is added.
    pub fn toggle_confirm_playlist(&mut self) {
        self.confirm_playlist = !self.confirm_playlist;
        self.save_state();
        self.message = match self.confirm_playlist {
            true => "playlists ask before downloading".into(),
            false => "playlists download every entry".into(),
        };
    }

    /// Where new downloads are written. Running transfers keep their old dir.
    pub fn set_dir(&mut self, dir: &str) {
        let path = PathBuf::from(expand_home(dir.trim()));
        // A directory that is not there is a typo, not an instruction to
        // create one four levels deep.
        if !path.is_dir() {
            self.message = format!("not a directory: {}", path.display());
            return;
        }
        // It exists, but that is not the same as being able to write in it.
        if let Err(e) = crate::utils::prepare_dir(&path) {
            self.message = e;
            return;
        }
        self.dir = path;
        self.save_state();
        self.message = format!("saving to {}", self.dir.display());
    }

    /// Everything a channel has uploaded since it was last synced, queued
    /// into the current queue. `only` names one channel and opens the picker
    /// for it; `None` syncs every channel and queues what it finds, since one
    /// picker cannot show several channels at once.
    ///
    /// The last-sync date moves to today whether or not the listing is
    /// picked from, so a sync is never repeated by accident. The file is the
    /// state, and the panel writes it before this runs.
    pub fn sync_channels(&mut self, only: Option<usize>) {
        let mut channels = channel::load();
        let today = crate::utils::today();
        let mut synced = 0;
        for (at, c) in channels.iter_mut().enumerate() {
            if only.is_some_and(|one| one != at) {
                continue;
            }
            // Routed on the channel's own url, as a playlist added by hand
            // is: its entries are `watch?v=…` links that match no rule
            // written about a channel, so the queue and directory a rule
            // gives it have to be decided here and handed down.
            let mut over = Overrides::default();
            let queue = self.route(&c.url, self.queue().id, &mut over);
            self.list_playlist(
                Listing {
                    url: c.url.clone(),
                    queue,
                    over,
                    // An empty `after` is the first sync: everything there is.
                    dates: DateRange { after: c.last_sync.clone(), before: String::new() },
                    entries: Vec::new(),
                },
                only.is_some(),
            );
            c.last_sync = today.clone();
            synced += 1;
        }
        let _ = channel::save(&channels);
        match (synced, only) {
            (0, _) => self.message = "no channels to sync — add one in settings".into(),
            // One channel's message is the listing's own; it ends in a picker.
            (_, Some(_)) => {}
            _ => self.message = format!("syncing {synced} channels — this takes a while…"),
        }
    }

    /// Persist the directory and queues. Called by every action that changes
    /// them, so there is no separate "save settings" step to forget.
    pub fn save_state(&self) {
        State::save(&self.dir, &self.queues, &self.downloads, self.nerd, self.confirm_playlist);
    }
}



/// Panel-only helpers.
impl App {
    /// Close the settings panel, saving the backend form it was showing.
    pub fn close_settings(&mut self) {
        let Some(panel) = self.settings.take() else { return };
        // A rule that decides nothing would swallow every url it matches, so
        // it is dropped rather than saved.
        let rules: Vec<Rule> = panel
            .rules
            .into_iter()
            .filter(|r| r.queue.is_some() || r.directory.is_some() || r.backend.is_some())
            .collect();
        let saved = rule::save(&rules);
        // The rules the app routes by, without waiting for a restart.
        self.rules = rules;
        let channels = channel::save(&panel.channels);
        // Every form is a file, and all of them are written on the way out.
        self.message = match panel.options.save().and(panel.crawl.save()).and(saved).and(channels) {
            Ok(()) => format!("{} options saved", panel.options.backend),
            Err(e) => format!("could not save options: {e}"),
        };
    }
}
