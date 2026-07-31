use crossterm::event::KeyCode;

use crate::controllers::app::App;
use crate::controllers::downloads::Filter;
use crate::controllers::options::Options;

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

/// One entry of a which-key style menu: press the prefix, then this key.
pub struct MenuItem {
    pub key: char,
    pub label: &'static str,
}

/// Queue commands, reached with the `g` prefix (`gn`, `gr`, …); `q` stays
/// quit. The menu popover renders this table, so an entry documents itself.
pub const QUEUE_MENU: &[MenuItem] = &[
    MenuItem { key: 'n', label: "new queue" },
    MenuItem { key: 'r', label: "rename queue" },
    MenuItem { key: 'd', label: "delete queue" },
    MenuItem { key: 'p', label: "pause / resume this queue" },
    MenuItem { key: 'P', label: "pause / resume every queue" },
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
    MenuItem { key: 'a', label: "aria2c options" },
    MenuItem { key: 'y', label: "yt-dlp options" },
];

pub fn menu_for(prefix: char) -> Option<(&'static str, &'static [MenuItem])> {
    match prefix {
        'g' => Some(("queue", QUEUE_MENU)),
        's' => Some(("settings", SETTINGS_MENU)),
        _ => None,
    }
}

fn char_of(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

/// Keyboard routing: panel, then half-typed sequence, then dialog, then the
/// normal keymap. Each layer owns the keyboard while it is active.
impl App {
    /// Handle one keypress. Returns true when the app should quit.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        if let Some(panel) = &mut self.options {
            if panel.on_key(key) {
                let panel = self.options.take().expect("open above");
                self.message = match panel.save() {
                    Ok(()) => format!("{} options saved", panel.backend),
                    Err(e) => format!("could not save {} options: {e}", panel.backend),
                };
            }
            return false;
        }

        // A half-typed sequence swallows the next key, vim style.
        if let Some(prefix) = self.pending.take() {
            return self.on_sequence(prefix, key);
        }

        match self.dialog.take() {
            Some(dialog) => self.on_dialog_key(dialog, key),
            None => return self.on_normal_key(key),
        }
        false
    }

    fn on_dialog_key(&mut self, dialog: Dialog, key: KeyCode) {
        match dialog {
            Dialog::Delete(at) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::Delete(at)),
            },
            Dialog::QueueDelete(at) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete_queue(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::QueueDelete(at)),
            },
            Dialog::Add(buf) => {
                if let Some(text) = self.type_into(buf, key, Dialog::Add) {
                    self.add(&text);
                }
            }
            Dialog::Edit(at, buf) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::Edit(at, b)) {
                    self.edit(at, &text);
                }
            }
            Dialog::SetDir(buf) => {
                if let Some(text) = self.type_into(buf, key, Dialog::SetDir) {
                    self.set_dir(&text);
                }
            }
            Dialog::QueueNew(buf) => {
                if let Some(text) = self.type_into(buf, key, Dialog::QueueNew) {
                    self.add_queue(&text);
                }
            }
            Dialog::QueueRename(at, buf) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::QueueRename(at, b)) {
                    self.rename_queue(at, &text);
                }
            }
        }
    }

    fn on_normal_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Char(c @ ('g' | 's' | 'Z')) => self.pending = Some(c),
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
            KeyCode::Char('p') | KeyCode::Char(' ') => self.toggle_pause(self.selected),
            KeyCode::Char('P') => self.toggle_all_pause(),
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
            ('s', 'd') => self.dialog = Some(Dialog::SetDir(self.dir.display().to_string())),
            ('s', 'a') => self.options = Some(Options::open("aria2c")),
            ('s', 'y') => self.options = Some(Options::open("yt-dlp")),
            ('g', 'n') => self.dialog = Some(Dialog::QueueNew(String::new())),
            ('g', 'r') => {
                self.dialog = Some(Dialog::QueueRename(self.current, self.queue().name.clone()))
            }
            ('g', 'd') => self.dialog = Some(Dialog::QueueDelete(self.current)),
            ('g', 'p') => self.toggle_queue_pause(),
            ('g', 'P') => self.toggle_all_pause(),
            ('g', 'j') | ('g', 'l') => self.cycle_queue(1),
            ('g', 'k') | ('g', 'h') => self.cycle_queue(-1),
            ('g', '+') | ('g', '=') => self.set_max_active(self.queue().max_active + 1),
            ('g', '-') => self.set_max_active(self.queue().max_active - 1),
            _ => {}
        }
        false
    }

    /// Text-field keys shared by every input dialog. Returns the text on
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
