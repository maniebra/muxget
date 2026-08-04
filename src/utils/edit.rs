use crossterm::event::{KeyCode, KeyModifiers};

/// One line of text being typed into, edited in place. `at` is a byte index
/// into `text` and always lands on a character boundary.
///
/// Every field in muxget goes through this — the add form, the settings
/// panel, every dialog — so a key that works in one works in all of them.
/// Returns false when the key means nothing here, which is how the caller
/// gets Enter, Esc and Tab back to do its own thing with.
///
/// The bindings are the ones a terminal has trained everybody to expect:
/// readline's, which a shell and a browser address bar both answer to.
pub fn key(text: &mut String, at: &mut usize, key: KeyCode, mods: KeyModifiers) -> bool {
    let alt = mods.contains(KeyModifiers::ALT);
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    // `usize::MAX` is how a field says "start at the end", and a shrinking
    // text can leave the caret past it.
    *at = (*at).min(text.len());
    match key {
        // Shift is how a capital arrives, so only the two chord modifiers
        // keep a character from being one.
        KeyCode::Char(c) if !ctrl && !alt => {
            text.insert(*at, c);
            *at += c.len_utf8();
        }
        KeyCode::Left if ctrl || alt => *at = word_start(text, *at),
        KeyCode::Right if ctrl || alt => *at = word_end(text, *at),
        KeyCode::Left => *at = back(text, *at),
        KeyCode::Right => *at = forward(text, *at),
        KeyCode::Home => *at = 0,
        KeyCode::End => *at = text.len(),
        KeyCode::Char('a') if ctrl => *at = 0,
        KeyCode::Char('e') if ctrl => *at = text.len(),
        // A word back. `/` and `.` are not word characters, so this takes one
        // path segment of a url rather than the whole thing.
        KeyCode::Backspace if alt || ctrl => cut(text, at, word_start(text, *at)),
        // The whole word, spaces in — what a shell's Ctrl-W does.
        KeyCode::Char('w') if ctrl => cut(text, at, space_start(text, *at)),
        KeyCode::Backspace => cut(text, at, back(text, *at)),
        KeyCode::Delete if alt || ctrl => {
            let to = word_end(text, *at);
            text.replace_range(*at..to, "");
        }
        KeyCode::Delete => {
            let to = forward(text, *at);
            text.replace_range(*at..to, "");
        }
        KeyCode::Char('u') if ctrl => cut(text, at, 0),
        KeyCode::Char('k') if ctrl => text.truncate(*at),
        _ => return false,
    }
    true
}

/// The text as it is drawn, with the caret where the next character lands.
pub fn caret(text: &str, at: usize) -> String {
    let at = at.min(text.len());
    format!("{}▏{}", &text[..at], &text[at..])
}

/// Delete from `from` up to the caret, and leave the caret where the deleted
/// run started.
fn cut(text: &mut String, at: &mut usize, from: usize) {
    text.replace_range(from..*at, "");
    *at = from;
}

fn back(text: &str, at: usize) -> usize {
    text[..at].chars().next_back().map_or(0, |c| at - c.len_utf8())
}

fn forward(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(at, |c| at + c.len_utf8())
}

/// The start of the word behind the caret: over any separators first, then
/// over the word itself, so a second press keeps going.
fn word_start(text: &str, at: usize) -> usize {
    let at = back_while(text, at, |c| !c.is_alphanumeric());
    back_while(text, at, char::is_alphanumeric)
}

fn word_end(text: &str, at: usize) -> usize {
    let at = forward_while(text, at, |c| !c.is_alphanumeric());
    forward_while(text, at, char::is_alphanumeric)
}

/// The start of the whitespace-delimited word behind the caret.
fn space_start(text: &str, at: usize) -> usize {
    let at = back_while(text, at, char::is_whitespace);
    back_while(text, at, |c| !c.is_whitespace())
}

fn back_while(text: &str, mut at: usize, keep: impl Fn(char) -> bool) -> usize {
    while let Some(c) = text[..at].chars().next_back() {
        if !keep(c) {
            break;
        }
        at -= c.len_utf8();
    }
    at
}

fn forward_while(text: &str, mut at: usize, keep: impl Fn(char) -> bool) -> usize {
    while let Some(c) = text[at..].chars().next() {
        if !keep(c) {
            break;
        }
        at += c.len_utf8();
    }
    at
}
