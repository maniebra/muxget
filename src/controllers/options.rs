use crossterm::event::KeyCode;

use crate::models::channel::{self, Channel};
use crate::models::option::{specs, Kind, OptSpec, Preset};
use crate::models::log;
use crate::models::rule::{self, Rule};
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

    /// The preset a flag's current value names, if it is one of them.
    pub fn preset<'a>(&self, flag: &str, presets: &'a [Preset]) -> Option<&'a Preset> {
        let value = self.value(flag)?;
        presets.iter().find(|p| p.value == value)
    }

    /// Step to the next preset, wrapping. An unset flag — or one holding a
    /// hand-written value — lands on the first one rather than losing its
    /// place, so cycling never silently drops what someone typed twice.
    fn cycle(&mut self, flag: &str, presets: &[Preset]) {
        let next = match self.preset(flag, presets) {
            Some(current) => {
                let at = presets.iter().position(|p| p == current).unwrap_or(0);
                (at + 1) % presets.len()
            }
            None => 0,
        };
        self.set(flag, presets[next].value);
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
        let flag = spec.flag;
        let kind_is_value = spec.kind == Kind::Value;
        let presets = match spec.kind {
            Kind::Choice(presets) => Some(presets),
            _ => None,
        };

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
            KeyCode::Enter | KeyCode::Char(' ') => match presets {
                Some(presets) => self.cycle(flag, presets),
                None if kind_is_value => {
                    self.editing = Some(self.value(flag).unwrap_or_default().to_string())
                }
                None => self.toggle(flag),
            },
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
    /// The crawler tab: the same form over `crawl.args`, which holds the
    /// defaults the crawl dialog opens with.
    pub crawl: Options,
    /// The categories tab edits a copy; the app takes it back on close, so a
    /// half-typed rule never routes anything.
    pub rules: Vec<Rule>,
    /// Rule field being typed into, as (row, text).
    pub editing_rule: Option<(usize, String)>,
    /// Channels to keep up with, edited here and written on the way out.
    pub channels: Vec<Channel>,
    /// Channel field being typed into, as (row, the date rather than the url,
    /// text).
    pub editing_channel: Option<(usize, bool, String)>,
}

/// Rows on the categories tab: one header per rule, then its fields.
pub const RULE_ROWS: usize = 1 + rule::FIELDS.len();

pub const TABS: [&str; 6] =
    ["general", "backends", "crawler", "categories", "channels", "log"];
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
    /// Sync the channel at this index, picking from what it finds.
    SyncChannel(usize),
    /// Sync every channel, queueing everything since each one's last sync.
    SyncChannels,
}

impl Settings {
    pub fn open(tab: usize, backend: &'static str, rules: Vec<Rule>) -> Settings {
        Settings {
            tab,
            cursor: 0,
            options: Options::open(backend),
            crawl: Options::open("crawl"),
            rules,
            editing_rule: None,
            channels: channel::load(),
            editing_channel: None,
        }
    }

    /// The rule a row belongs to, and which of its fields the row is — `None`
    /// for the header row that names the rule.
    pub fn row_at(&self, row: usize) -> Option<(usize, Option<usize>)> {
        let at = row / RULE_ROWS;
        (at < self.rules.len()).then(|| (at, (row % RULE_ROWS).checked_sub(1)))
    }

    /// The form the current tab shows, if it is one of the two that has one.
    pub fn form(&mut self) -> Option<&mut Options> {
        match self.tab {
            1 => Some(&mut self.options),
            2 => Some(&mut self.crawl),
            _ => None,
        }
    }

    pub fn rows(&self) -> usize {
        match self.tab {
            0 => GENERAL.len(),
            1 => self.options.specs().len(),
            2 => self.crawl.specs().len(),
            3 => self.rules.len() * RULE_ROWS,
            4 => self.channels.len(),
            _ => log::entries().len(),
        }
    }

    /// A rule's field was typed into and accepted; empty clears the field.
    fn accept_rule(&mut self, row: usize, text: &str) {
        let Some((at, Some(field))) = self.row_at(row) else { return };
        self.rules[at].set(field, text);
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

    /// A channel's url or last-sync date was typed into. An empty url deletes
    /// the channel — there is nothing left to sync.
    fn accept_channel(&mut self, row: usize, date: bool, text: &str) {
        let Some(channel) = self.channels.get_mut(row) else { return };
        let text = text.trim();
        match date {
            // Stored the way yt-dlp reads one, however it was typed.
            true => channel.last_sync = crate::models::ytdlp::date(text),
            false => channel.url = text.to_string(),
        }
        if channel.url.is_empty() {
            self.channels.remove(row);
            self.cursor = row.min(self.rows().saturating_sub(1));
        }
    }

    pub fn on_key(&mut self, key: KeyCode) -> Action {
        // A channel field being typed into owns the keyboard.
        if let Some((row, date, mut buf)) = self.editing_channel.take() {
            match key {
                KeyCode::Enter => self.accept_channel(row, date, &buf),
                // Cancelled, but a row added with `n` and never given a url
                // is not a channel and must not be left behind.
                KeyCode::Esc => {
                    let url = self.channels.get(row).map(|c| c.url.clone()).unwrap_or_default();
                    self.accept_channel(row, false, &url);
                }
                KeyCode::Backspace => {
                    buf.pop();
                    self.editing_channel = Some((row, date, buf));
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.editing_channel = Some((row, date, buf));
                }
                _ => self.editing_channel = Some((row, date, buf)),
            }
            return Action::None;
        }

        // A rule field being typed into owns the keyboard.
        if let Some((row, mut buf)) = self.editing_rule.take() {
            match key {
                KeyCode::Enter => self.accept_rule(row, &buf),
                KeyCode::Esc => {}
                KeyCode::Backspace => {
                    buf.pop();
                    self.editing_rule = Some((row, buf));
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.editing_rule = Some((row, buf));
                }
                _ => self.editing_rule = Some((row, buf)),
            }
            return Action::None;
        }

        // A form owns the keyboard while a value is being typed into it.
        let cursor = self.cursor;
        if let Some(form) = self.form().filter(|f| f.editing.is_some()) {
            form.cursor = cursor;
            form.on_key(key);
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
            // The categories tab is a list of rules, each unfolded into its
            // fields: `n` adds one, `x` clears a field or deletes a rule.
            (3, row, KeyCode::Char('n')) => {
                let at = self.row_at(row).map_or(self.rules.len(), |(at, _)| at + 1);
                self.rules.insert(at, Rule::default());
                self.cursor = at * RULE_ROWS;
                Action::None
            }
            (3, row, KeyCode::Enter) => {
                if let Some((at, Some(field))) = self.row_at(row) {
                    self.editing_rule = Some((row, self.rules[at].get(field)));
                }
                Action::None
            }
            (3, row, KeyCode::Char('x')) | (3, row, KeyCode::Delete) => {
                match self.row_at(row) {
                    Some((at, Some(field))) => self.rules[at].set(field, ""),
                    Some((at, None)) => {
                        self.rules.remove(at);
                        self.cursor = row.min(self.rows().saturating_sub(1));
                    }
                    None => {}
                }
                Action::None
            }
            // Channels: one row each, `s` syncs it, `S` syncs the lot.
            (4, row, KeyCode::Char('n')) => {
                let at = (row + 1).min(self.channels.len());
                self.channels.insert(at, Channel::default());
                self.cursor = at;
                self.editing_channel = Some((at, false, String::new()));
                Action::None
            }
            (4, row, KeyCode::Enter) => {
                if let Some(c) = self.channels.get(row) {
                    self.editing_channel = Some((row, false, c.url.clone()));
                }
                Action::None
            }
            (4, row, KeyCode::Char('d')) => {
                if let Some(c) = self.channels.get(row) {
                    self.editing_channel = Some((row, true, c.last_sync.clone()));
                }
                Action::None
            }
            (4, row, KeyCode::Char('x') | KeyCode::Delete) => {
                if row < self.channels.len() {
                    self.channels.remove(row);
                    self.cursor = row.min(self.rows().saturating_sub(1));
                }
                Action::None
            }
            (4, row, KeyCode::Char('s')) if row < self.channels.len() => Action::SyncChannel(row),
            (4, _, KeyCode::Char('S')) => Action::SyncChannels,
            // The log is read, not edited; `x` empties it, `G` jumps to the
            // newest line, which is the one worth looking at first.
            (5, _, KeyCode::Char('x')) => {
                log::clear();
                self.cursor = 0;
                Action::None
            }
            (5, _, KeyCode::Char('G') | KeyCode::End) => {
                self.cursor = self.rows().saturating_sub(1);
                Action::None
            }
            (5, _, KeyCode::Char('g') | KeyCode::Home) => {
                self.cursor = 0;
                Action::None
            }
            (1 | 2, _, key) => {
                let cursor = self.cursor;
                if let Some(form) = self.form() {
                    form.cursor = cursor;
                    form.on_key(key);
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}
