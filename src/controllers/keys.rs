use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Rect, Size};

use crate::controllers::app::{App, Help};
use crate::controllers::downloads::Filter;
use crate::views::ui;
use crate::controllers::crawl;
use crate::controllers::options::{Action, Settings};
use crate::models::crawl::{wild, Crawl, Found};
use crate::models::ytdlp::Listing;
use crate::models::download::Overrides;
use crate::utils;

/// The add dialog's fields, in display order.
pub const FORM_LABELS: [&str; 7] = [
    "url",
    "range",
    "directory",
    "file name",
    "rate limit",
    "user",
    "password",
];

/// Fields shown as dots rather than text.
pub const SECRET_FIELDS: [bool; 7] = [false, false, false, false, false, false, true];

/// A url plus the settings for that download alone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Form {
    pub fields: [String; 7],
    /// Field being typed into.
    pub cursor: usize,
}

impl Form {
    pub fn overrides(&self) -> Overrides {
        Overrides {
            dir: self.fields[2].trim().to_string(),
            name: self.fields[3].trim().to_string(),
            rate: self.fields[4].trim().to_string(),
            // Only a rule or a crawl sets these; the form has no field.
            backend: String::new(),
            args: String::new(),
            user: self.fields[5].trim().to_string(),
            // Not trimmed: a password's spaces are part of it.
            pass: self.fields[6].clone(),
        }
    }

    /// Every url this form will add: one per number in the range.
    pub fn urls(&self) -> Vec<String> {
        utils::expand(self.fields[0].trim(), &self.fields[1])
    }
}

/// Modal state. `Some` means the popover is up and owns the keyboard.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    /// New download being filled in.
    Add(Form),
    /// Editing the url of the download at this index; edit restarts it.
    Edit(usize, String),
    /// Confirming removal of the download at this index.
    Delete(usize),
    /// Confirming removal of the download at this index *and* its file.
    DeleteData(usize),
    /// Name for a new queue.
    QueueNew(String),
    /// Renaming the queue at this index in `queues`.
    QueueRename(usize, String),
    /// Confirming removal of the queue at this index in `queues`.
    QueueDelete(usize),
    /// Confirming a clear of the queue at this index: every row when the flag
    /// is set, the finished ones otherwise.
    QueueClear(usize, bool),
    /// Daily active window for the queue at this index, `HH:MM-HH:MM`.
    QueueSchedule(usize, String),
    /// New download directory being typed.
    SetDir(String),
    /// A crawl being described.
    Crawl(Form),
    /// What a crawl found: the crawl, its links, which are picked, and the
    /// row the cursor is on.
    Crawled(Box<Crawl>, Vec<Found>, Vec<usize>, usize),
    /// A listed playlist waiting to be picked from.
    Playlist(Box<Pick>),
    /// Urls found in the clipboard, waiting to be confirmed: the urls, which
    /// are picked, and the row the cursor is on.
    Paste(Vec<String>, Vec<usize>, usize),
}

/// A playlist listed but not yet queued: its entries, which of them are
/// picked, and the settings they will be queued with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pick {
    pub listing: Listing,
    /// Entries picked, as indexes into `listing.entries`.
    pub picked: Vec<usize>,
    /// Row the cursor is on, as a position in `shown`.
    pub at: usize,
    /// Words the title must contain; `-word` must not appear and `*` is a
    /// wildcard. Rows that do not match are hidden and stay unpicked.
    pub words: String,
    /// Field being typed into, while it is being typed.
    pub editing: Option<(Field, String)>,
}

/// A picker field that takes text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Field {
    Dir,
    Words,
    /// The two ends of the upload-date range, each typed on its own.
    From,
    To,
}

impl Pick {
    /// Entries matching the filters, as indexes. Everything when nothing is
    /// typed, so this is the list to walk in every case.
    pub fn shown(&self) -> Vec<usize> {
        let words: Vec<&str> = self.words.split_whitespace().collect();
        let dates = &self.listing.dates;
        (0..self.listing.entries.len())
            .filter(|i| {
                let entry = &self.listing.entries[*i];
                let text = match entry.title.is_empty() {
                    true => entry.url.to_lowercase(),
                    false => entry.title.to_lowercase(),
                };
                let by_words = words.iter().all(|word| match word.strip_prefix('-') {
                    Some(word) => !wild(word, &text),
                    None => wild(word, &text),
                });
                // `YYYYMMDD` sorts as text, so a range is two comparisons. An
                // entry the listing gave no date for is kept rather than
                // hidden by a filter it cannot answer.
                let by_date = entry.date.is_empty()
                    || ((dates.after.is_empty() || entry.date >= dates.after)
                        && (dates.before.is_empty() || entry.date <= dates.before));
                by_words && by_date
            })
            .collect()
    }

    /// Entries the listing gave no date for, which no date filter can judge.
    pub fn undated(&self) -> usize {
        self.listing.entries.iter().filter(|e| e.date.is_empty()).count()
    }
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
    MenuItem { key: 'c', label: "clear finished rows" },
    MenuItem { key: 'C', label: "clear every row (also `C`)" },
    MenuItem { key: 'p', label: "pause / resume this queue" },
    MenuItem { key: 't', label: "schedule (window, days, quota…)" },
    MenuItem { key: 'P', label: "pause / resume every queue" },
    MenuItem { key: 'g', label: "first row" },
    MenuItem { key: 'j', label: "next queue" },
    MenuItem { key: 'k', label: "previous queue" },
    MenuItem { key: '>', label: "move this queue right" },
    MenuItem { key: '<', label: "move this queue left" },
    MenuItem { key: '+', label: "one more slot" },
    MenuItem { key: '-', label: "one less slot" },
];

/// Commands for the selected row, reached with the `i` prefix.
pub const ITEM_MENU: &[MenuItem] = &[
    MenuItem { key: 'r', label: "remove" },
    MenuItem { key: 'R', label: "remove and delete the file" },
    MenuItem { key: 'o', label: "open" },
    MenuItem { key: 'f', label: "open containing folder" },
    MenuItem { key: 'F', label: "force restart (torrents)" },
    MenuItem { key: 't', label: "retry (failed or cancelled)" },
];

pub fn menu_for(prefix: char) -> Option<(&'static str, &'static [MenuItem])> {
    match prefix {
        'g' => Some(("queue", QUEUE_MENU)),
        'i' => Some(("item", ITEM_MENU)),
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
    /// One key with its modifiers. Only the control chords need them, and
    /// only in the list — a dialog takes the plain key as it always has.
    pub fn on_key_event(&mut self, ev: KeyEvent) -> bool {
        let plain = self.dialog.is_none() && self.settings.is_none() && self.pending.is_none();
        if plain && ev.modifiers.contains(KeyModifiers::CONTROL) {
            let page = self.page as isize;
            match ev.code {
                // vim's paging: half a screen, then a whole one.
                KeyCode::Char('d') => self.move_selection(page / 2),
                KeyCode::Char('u') => self.move_selection(-page / 2),
                KeyCode::Char('f') => self.move_selection(page),
                KeyCode::Char('b') => self.move_selection(-page),
                _ => {}
            }
            if matches!(ev.code, KeyCode::Char('d' | 'u' | 'f' | 'b')) {
                self.count = None;
                return false;
            }
        }
        self.on_key(ev.code)
    }

    /// Handle one keypress. Returns true when the app should quit.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        if self.help.is_some() {
            self.help_key(key);
            return false;
        }
        if let Some(panel) = &mut self.settings {
            match panel.on_key(key) {
                Action::None => {}
                Action::Close => self.close_settings(),
                Action::NextTheme => self.set_theme(self.theme.next(&self.themes)),
                Action::PrevTheme => self.set_theme(self.theme.prev(&self.themes)),
                Action::ToggleNerd => self.toggle_nerd(),
                Action::ToggleConfirmPlaylist => self.toggle_confirm_playlist(),
                Action::EditDir => {
                    self.close_settings();
                    self.dialog = Some(Dialog::SetDir(self.dir.display().to_string()));
                }
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
                KeyCode::Enter | KeyCode::Char('y') => {
                    self.on_targets(|app, at| app.delete(at))
                }
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::Delete(at)),
            },
            Dialog::DeleteData(at) => match key {
                KeyCode::Enter | KeyCode::Char('y') => {
                    self.on_targets(|app, at| app.delete_with_data(at))
                }
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::DeleteData(at)),
            },
            Dialog::QueueClear(at, all) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.clear_queue(at, all),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::QueueClear(at, all)),
            },
            Dialog::QueueDelete(at) => match key {
                KeyCode::Enter | KeyCode::Char('y') => self.delete_queue(at),
                KeyCode::Esc | KeyCode::Char('n') => {}
                _ => self.dialog = Some(Dialog::QueueDelete(at)),
            },
            Dialog::Add(mut form) => {
                if !type_in_form(&mut form, key) {
                    self.dialog = Some(Dialog::Add(form));
                    return;
                }
                if key != KeyCode::Enter {
                    return;
                }
                let (over, range) = (form.overrides(), form.fields[1].clone());
                for (i, url) in form.urls().iter().enumerate() {
                    // Or every item in the range writes to one file.
                    let mut over = over.clone();
                    if let Some((from, _)) = utils::parse_range(&range) {
                        over.name = utils::fill(&over.name, from + i as i64);
                    }
                    self.add_with(url, over);
                }
            }
            Dialog::Crawl(mut form) => {
                if !type_in_form(&mut form, key) {
                    self.dialog = Some(Dialog::Crawl(form));
                    return;
                }
                if key == KeyCode::Enter {
                    self.start_crawl(crawl::from_form(&form));
                }
            }
            Dialog::Crawled(crawl, found, mut picked, mut at) => {
                match key {
                    KeyCode::Enter => {
                        picked.sort_unstable();
                        self.add_found(&crawl, &found, &picked);
                        return;
                    }
                    KeyCode::Esc => return,
                    key => pick_nav(key, found.len(), &mut picked, &mut at),
                }
                self.dialog = Some(Dialog::Crawled(crawl, found, picked, at));
            }
            Dialog::Playlist(mut pick) => {
                // A field being typed into owns the keyboard until Enter or Esc.
                if let Some((field, mut buf)) = pick.editing.take() {
                    match key {
                        KeyCode::Enter => {
                            if self.typed_into_picker(&mut pick, field, &buf) {
                                // A new date range means listing again, and
                                // the new list brings its own picker.
                                return;
                            }
                        }
                        KeyCode::Esc => {}
                        KeyCode::Backspace => {
                            buf.pop();
                            pick.editing = Some((field, buf));
                        }
                        KeyCode::Char(c) => {
                            buf.push(c);
                            pick.editing = Some((field, buf));
                        }
                        _ => pick.editing = Some((field, buf)),
                    }
                    self.dialog = Some(Dialog::Playlist(pick));
                    return;
                }
                match key {
                    KeyCode::Enter => {
                        self.add_listed(&pick);
                        return;
                    }
                    KeyCode::Esc => return,
                    KeyCode::Char('d') => {
                        pick.editing = Some((Field::Dir, pick.listing.over.dir.clone()))
                    }
                    KeyCode::Char('/') => pick.editing = Some((Field::Words, pick.words.clone())),
                    KeyCode::Char('t') => {
                        pick.editing = Some((Field::From, pick.listing.dates.after.clone()))
                    }
                    KeyCode::Char('T') => {
                        pick.editing = Some((Field::To, pick.listing.dates.before.clone()))
                    }
                    // The list walked is the filtered one, so the cursor and
                    // `a` only ever touch rows that are on screen.
                    key => {
                        let shown = pick.shown();
                        let mut at_row = pick.picked.iter().filter(|i| shown.contains(i)).count();
                        pick_row(key, &shown, &mut pick.picked, &mut pick.at, &mut at_row);
                    }
                }
                self.dialog = Some(Dialog::Playlist(pick));
            }
            Dialog::Paste(urls, mut picked, mut at) => {
                match key {
                    KeyCode::Enter => {
                        picked.sort_unstable();
                        self.add_pasted(&urls, &picked);
                        return;
                    }
                    KeyCode::Esc => return,
                    key => pick_nav(key, urls.len(), &mut picked, &mut at),
                }
                self.dialog = Some(Dialog::Paste(urls, picked, at));
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
            Dialog::QueueSchedule(at, buf) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::QueueSchedule(at, b)) {
                    self.set_schedule(at, &text);
                }
            }
            Dialog::QueueRename(at, buf) => {
                if let Some(text) = self.type_into(buf, key, |b| Dialog::QueueRename(at, b)) {
                    self.rename_queue(at, &text);
                }
            }
        }
    }

    /// The manual owns the keyboard while it is open: pages sideways, scrolls
    /// down, and closes on anything that means "done".
    fn help_key(&mut self, key: KeyCode) {
        let Some(mut help) = self.help else { return };
        let pages = crate::views::help::PAGES.len();
        let lines = crate::views::help::PAGES[help.tab].lines.len();
        match key {
            KeyCode::Esc | KeyCode::Char('q' | '?') => {
                self.help = None;
                return;
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                help = Help { tab: (help.tab + 1) % pages, scroll: 0 }
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                help = Help { tab: (help.tab + pages - 1) % pages, scroll: 0 }
            }
            // Never scroll the last line off the top.
            KeyCode::Down | KeyCode::Char('j') => {
                help.scroll = (help.scroll + 1).min(lines.saturating_sub(1))
            }
            KeyCode::Up | KeyCode::Char('k') => help.scroll = help.scroll.saturating_sub(1),
            KeyCode::Char('g') => help.scroll = 0,
            _ => {}
        }
        self.help = Some(help);
    }

    /// The count typed before a movement, and gone once it is used. `0` only
    /// counts when a count is already going, so it stays free for a `0` key.
    fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    fn on_normal_key(&mut self, key: KeyCode) -> bool {
        // Digits build a count for the next movement rather than acting.
        if let KeyCode::Char(c @ '0'..='9') = key {
            let digit = c as usize - '0' as usize;
            if digit > 0 || self.count.is_some() {
                self.count = Some(self.count.unwrap_or(0) * 10 + digit);
                return false;
            }
        }
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Char(c @ ('g' | 'i' | 'Z')) => self.pending = Some(c),
            // Settings are one panel, not a menu of them.
            KeyCode::Char('s') => {
                self.settings = Some(Settings::open(0, "aria2c", self.rules.clone()))
            }
            KeyCode::Char('a') => self.dialog = Some(Dialog::Add(Form::default())),
            KeyCode::Char('c') => self.dialog = Some(Dialog::Crawl(Form::default())),
            KeyCode::Char('v') => self.paste(),
            KeyCode::Char('?') => self.help = Some(Help::default()),
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
            KeyCode::Char('m') | KeyCode::Char(' ') => self.mark(),
            KeyCode::Char('M') => self.mark_range(),
            KeyCode::Char('A') => self.mark_all(),
            KeyCode::Char('p') => self.on_targets(|app, at| app.toggle_pause(at)),
            KeyCode::Char('C') => self.ask_clear(true),
            KeyCode::Char('P') => self.toggle_all_pause(),
            KeyCode::Char('x') => self.on_targets(|app, at| app.cancel(at)),
            KeyCode::Char(']') | KeyCode::Right => self.cycle_queue(1),
            KeyCode::Char('[') | KeyCode::Left => self.cycle_queue(-1),
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.take_count() as isize;
                self.move_selection(n);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let n = self.take_count() as isize;
                self.move_selection(-n);
            }
            // `G` alone is the last row, `5G` the fifth.
            KeyCode::Char('G') | KeyCode::End => match self.count.take() {
                Some(n) => self.goto_row(n.saturating_sub(1)),
                None => self.goto_row(usize::MAX),
            },
            KeyCode::Home => self.goto_row(0),
            // Order is priority, so this is how a row is promoted.
            KeyCode::Char('J') => self.move_download(1),
            KeyCode::Char('K') => self.move_download(-1),
            KeyCode::Tab | KeyCode::Char('f') => {
                let i = Filter::ALL.iter().position(|f| *f == self.filter).unwrap_or(0);
                self.set_filter(Filter::ALL[(i + 1) % Filter::ALL.len()]);
            }
            _ => {}
        }
        // A count belongs to the command right after it, unless that command
        // is the first half of a sequence — `5gg` has to survive the `g`.
        if self.pending.is_none() {
            self.count = None;
        }
        false
    }

    /// Second key of a sequence. Anything unlisted quietly cancels, as in vim.
    fn on_sequence(&mut self, prefix: char, key: KeyCode) -> bool {
        let Some(c) = char_of(key) else { return false };
        match (prefix, c) {
            ('Z', 'Z') | ('Z', 'Q') => return true,
            ('i', 'r') => {
                if self.downloads.get(self.selected).is_some() {
                    self.dialog = Some(Dialog::Delete(self.selected));
                }
            }
            ('i', 'R') => {
                if self.downloads.get(self.selected).is_some() {
                    self.dialog = Some(Dialog::DeleteData(self.selected));
                }
            }
            ('i', 'F') => self.force_restart(self.selected),
            ('i', 't') => self.on_targets(|app, at| app.retry(at)),
            ('i', 'o') => self.open_item(self.selected),
            ('i', 'f') => self.reveal_item(self.selected),
            ('g', 'n') => self.dialog = Some(Dialog::QueueNew(String::new())),
            ('g', 'r') => {
                self.dialog = Some(Dialog::QueueRename(self.current, self.queue().name.clone()))
            }
            ('g', 'd') => self.dialog = Some(Dialog::QueueDelete(self.current)),
            ('g', 'c') => self.ask_clear(false),
            ('g', 'C') => self.ask_clear(true),
            ('g', 'p') => self.toggle_queue_pause(),
            ('g', 't') => {
                self.dialog =
                    Some(Dialog::QueueSchedule(self.current, self.queue().window()))
            }
            ('g', 'P') => self.toggle_all_pause(),
            ('g', 'g') => {
                let row = self.count.take().map_or(0, |n| n.saturating_sub(1));
                self.goto_row(row);
            }
            ('g', 'j') | ('g', 'l') => self.cycle_queue(1),
            ('g', 'k') | ('g', 'h') => self.cycle_queue(-1),
            ('g', '>') => self.move_queue(1),
            ('g', '<') => self.move_queue(-1),
            ('g', '+') | ('g', '=') => self.set_max_active(self.queue().max_active + 1),
            ('g', '-') => self.set_max_active(self.queue().max_active - 1),
            _ => {}
        }
        self.count = None;
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

/// Mouse routing. A dialog or panel owns the keyboard, and the mouse with it,
/// so clicks are ignored while one is open.
impl App {
    pub fn on_mouse(&mut self, ev: MouseEvent, size: Size) {
        if self.dialog.is_some() || self.settings.is_some() || self.pending.is_some() {
            return;
        }
        let area = Rect::new(0, 0, size.width, size.height);
        let panes = ui::layout(area);
        let (x, y) = (ev.column, ev.row);

        match ev.kind {
            MouseEventKind::ScrollDown => self.scroll(&panes, x, y, 1),
            MouseEventKind::ScrollUp => self.scroll(&panes, x, y, -1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(row) = row_at(panes.queues, x, y) {
                    if row < self.queues.len() {
                        self.current = row;
                        self.clamp_selection();
                    }
                } else if let Some(row) = row_at(panes.filters, x, y) {
                    if let Some(filter) = Filter::ALL.get(row) {
                        self.set_filter(*filter);
                    }
                } else if let Some(row) = row_at(Some(panes.list), x, y) {
                    // Row 0 is the header, which selects nothing.
                    if let Some(n) = row.checked_sub(1) {
                        let capacity = panes.list.height.saturating_sub(3) as usize;
                        self.select_visible(n, capacity);
                    }
                }
            }
            _ => {}
        }
    }

    fn scroll(&mut self, panes: &ui::Panes, x: u16, y: u16, delta: isize) {
        if row_at(panes.queues, x, y).is_some() {
            self.cycle_queue(delta);
        } else {
            self.move_selection(delta);
        }
    }

    /// Select the row `n` places down the visible list, counting from the top
    /// of what is on screen.
    fn select_visible(&mut self, n: usize, capacity: usize) {
        let rows = self.visible();
        let at = rows.iter().position(|i| *i == self.selected).unwrap_or(0);
        // ratatui gets a fresh TableState each frame, so it scrolls the least
        // it can: the selection sits at the bottom once the list overflows.
        let offset = at.saturating_sub(capacity.saturating_sub(1));
        if let Some(i) = rows.get(offset.saturating_add(n)) {
            self.selected = *i;
        }
    }
}

/// Which line of a bordered panel the pointer is on, if it is inside one.
fn row_at(area: Option<Rect>, x: u16, y: u16) -> Option<usize> {
    let area = area?;
    let inner = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(2));
    (x >= inner.x && x < inner.right() && y >= inner.y && y < inner.bottom())
        .then(|| (y - inner.y) as usize)
}

/// The picker's list keys, over the rows currently shown: the cursor is a
/// position in `shown`, while what is picked is entry indexes.
fn pick_row(
    key: KeyCode,
    shown: &[usize],
    picked: &mut Vec<usize>,
    at: &mut usize,
    shown_picked: &mut usize,
) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => *at = (*at + 1).min(shown.len().saturating_sub(1)),
        KeyCode::Up | KeyCode::Char('k') => *at = at.saturating_sub(1),
        KeyCode::Char(' ') => {
            let Some(entry) = shown.get(*at) else { return };
            match picked.iter().position(|i| i == entry) {
                Some(i) => {
                    picked.remove(i);
                }
                None => picked.push(*entry),
            }
        }
        KeyCode::Char('a') => {
            // All of what is shown, or none of it; rows hidden by the word
            // filter are left alone either way.
            picked.retain(|i| !shown.contains(i));
            if *shown_picked != shown.len() {
                picked.extend_from_slice(shown);
            }
        }
        _ => {}
    }
}

/// Cursor and selection keys shared by the two pick-from-a-list dialogs:
/// space picks one row, `a` every row or none.
pub fn pick_nav(key: KeyCode, len: usize, picked: &mut Vec<usize>, at: &mut usize) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => *at = (*at + 1).min(len.saturating_sub(1)),
        KeyCode::Up | KeyCode::Char('k') => *at = at.saturating_sub(1),
        KeyCode::Char(' ') => match picked.iter().position(|i| i == at) {
            Some(i) => {
                picked.remove(i);
            }
            None => picked.push(*at),
        },
        KeyCode::Char('a') => {
            *picked = match picked.len() == len {
                true => Vec::new(),
                false => (0..len).collect(),
            }
        }
        _ => {}
    }
}

/// One keypress into a multi-field form. Returns true when the form is done
/// with the key — Enter to submit, Esc to drop it — and false while it is
/// still being typed into.
fn type_in_form(form: &mut Form, key: KeyCode) -> bool {
    let fields = form.fields.len();
    match key {
        KeyCode::Enter | KeyCode::Esc => return true,
        KeyCode::Tab | KeyCode::Down => form.cursor = (form.cursor + 1) % fields,
        KeyCode::BackTab | KeyCode::Up => form.cursor = (form.cursor + fields - 1) % fields,
        KeyCode::Backspace => {
            form.fields[form.cursor].pop();
        }
        KeyCode::Char(c) => form.fields[form.cursor].push(c),
        _ => {}
    }
    false
}
