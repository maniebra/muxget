use crossterm::event::{KeyCode, KeyModifiers};
use muxget::utils::edit::{caret, key};

const ALT: KeyModifiers = KeyModifiers::ALT;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const NONE: KeyModifiers = KeyModifiers::NONE;

/// Type a string into a field, as a person would.
fn typed(text: &str, at: &mut usize, buf: &mut String) {
    for c in text.chars() {
        assert!(key(buf, at, KeyCode::Char(c), NONE));
    }
}

#[test]
fn text_is_inserted_and_deleted_at_the_caret_not_the_end() {
    let (mut buf, mut at) = (String::new(), usize::MAX);
    typed("helo world", &mut at, &mut buf);
    assert_eq!(at, buf.len(), "typing leaves the caret after what was typed");

    // Back to the missing letter and fix it in place.
    for _ in 0..7 {
        key(&mut buf, &mut at, KeyCode::Left, NONE);
    }
    typed("l", &mut at, &mut buf);
    assert_eq!(buf, "hello world");
    assert_eq!(caret(&buf, at), "hell▏o world", "and the caret is drawn there");

    // Backspace and Delete each take the character on their own side.
    key(&mut buf, &mut at, KeyCode::Backspace, NONE);
    key(&mut buf, &mut at, KeyCode::Delete, NONE);
    assert_eq!(buf, "hel world");
}

#[test]
fn a_word_at_a_time_stops_at_the_parts_of_a_url() {
    let (mut buf, mut at) = (String::new(), usize::MAX);
    typed("https://y.com/watch?v=abc", &mut at, &mut buf);

    // Alt-Backspace takes one part of the url, not the whole thing.
    assert!(key(&mut buf, &mut at, KeyCode::Backspace, ALT));
    assert_eq!(buf, "https://y.com/watch?v=");
    assert!(key(&mut buf, &mut at, KeyCode::Backspace, ALT));
    assert_eq!(buf, "https://y.com/watch?");

    // Ctrl-W is the shell's: back to the last space, so the lot goes.
    assert!(key(&mut buf, &mut at, KeyCode::Char('w'), CTRL));
    assert_eq!(buf, "");
}

#[test]
fn the_line_can_be_walked_and_cut_from_either_end() {
    let (mut buf, mut at) = (String::new(), usize::MAX);
    typed("one two three", &mut at, &mut buf);

    key(&mut buf, &mut at, KeyCode::Home, NONE);
    assert_eq!(at, 0);
    key(&mut buf, &mut at, KeyCode::Right, ALT);
    assert_eq!(caret(&buf, at), "one▏ two three", "a word forward");
    key(&mut buf, &mut at, KeyCode::Char('e'), CTRL);
    assert_eq!(at, buf.len(), "Ctrl-E is the end");

    // Ctrl-U cuts back to the start, Ctrl-K forward to the end.
    key(&mut buf, &mut at, KeyCode::Left, ALT);
    key(&mut buf, &mut at, KeyCode::Char('u'), CTRL);
    assert_eq!(buf, "three");
    key(&mut buf, &mut at, KeyCode::Right, NONE);
    key(&mut buf, &mut at, KeyCode::Char('k'), CTRL);
    assert_eq!(buf, "t");
}

#[test]
fn a_caret_inside_multibyte_text_stays_on_a_character() {
    let (mut buf, mut at) = (String::new(), usize::MAX);
    typed("naïve café", &mut at, &mut buf);
    for _ in 0..5 {
        key(&mut buf, &mut at, KeyCode::Left, NONE);
    }
    // A byte index that split `ï` or `é` would panic on the next slice.
    assert_eq!(caret(&buf, at), "naïve▏ café");
    key(&mut buf, &mut at, KeyCode::Backspace, NONE);
    assert_eq!(buf, "naïv café");
}

#[test]
fn keys_the_field_does_not_own_are_handed_back() {
    let (mut buf, mut at) = ("x".to_string(), usize::MAX);
    // The caller needs these to submit, cancel and move between fields.
    for key_code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Tab, KeyCode::Up, KeyCode::Down] {
        assert!(!key(&mut buf, &mut at, key_code, NONE), "{key_code:?} is not the field's");
    }
    // Nor are the chords that mean something to the list behind it.
    assert!(!key(&mut buf, &mut at, KeyCode::Char('d'), CTRL));
    assert_eq!(buf, "x", "and none of them changed the text");
}
