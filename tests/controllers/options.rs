use crossterm::event::KeyCode;
use muxget::controllers::options::Options;
use muxget::utils::args;

fn panel() -> Options {
    Options {
        backend: "aria2c",
        cursor: 0,
        pairs: Vec::new(),
        editing: None,
    }
}

fn typed(p: &mut Options, keys: &str) {
    for c in keys.chars() {
        p.on_key(KeyCode::Char(c));
    }
}

fn go_to(p: &mut Options, flag: &str) {
    p.cursor = p.specs().iter().position(|s| s.flag == flag).unwrap();
}

#[test]
fn toggling_a_flag_option_adds_and_removes_it() {
    let mut p = panel();
    go_to(&mut p, "--check-integrity");

    p.on_key(KeyCode::Enter);
    assert!(p.is_set("--check-integrity"));
    p.on_key(KeyCode::Enter);
    assert!(!p.is_set("--check-integrity"), "toggles back off");
}

#[test]
fn editing_a_value_option_writes_then_clears_it() {
    let mut p = panel();
    go_to(&mut p, "--split");

    p.on_key(KeyCode::Enter);
    assert_eq!(p.editing.as_deref(), Some(""), "starts empty when unset");
    typed(&mut p, "16");
    p.on_key(KeyCode::Enter);
    assert_eq!(p.value("--split"), Some("16"));

    // Reopening prefills, and Esc leaves the old value alone.
    p.on_key(KeyCode::Enter);
    assert_eq!(p.editing.as_deref(), Some("16"));
    typed(&mut p, "8");
    p.on_key(KeyCode::Esc);
    assert_eq!(p.value("--split"), Some("16"), "escape discards the edit");

    // An empty value unsets the option instead of writing `--split=`.
    p.on_key(KeyCode::Enter);
    for _ in 0..2 {
        p.on_key(KeyCode::Backspace);
    }
    p.on_key(KeyCode::Enter);
    assert!(!p.is_set("--split"));
}

#[test]
fn unknown_flags_survive_a_round_trip() {
    let mut p = panel();
    p.pairs = args::to_pairs(&args::parse("--split=8 --some-exotic-flag=1 --another"));

    assert_eq!(p.unknown().len(), 2, "flags without a spec are kept aside");

    go_to(&mut p, "--check-integrity");
    p.on_key(KeyCode::Enter);

    let text = args::render(&p.pairs);
    let back = args::parse(&text);
    assert!(back.contains(&"--some-exotic-flag=1".to_string()));
    assert!(back.contains(&"--another".to_string()));
    assert!(back.contains(&"--check-integrity".to_string()));
    assert!(back.contains(&"--split=8".to_string()));
}

#[test]
fn navigation_clamps_and_esc_closes() {
    let mut p = panel();
    let last = p.specs().len() - 1;

    p.on_key(KeyCode::Char('k'));
    assert_eq!(p.cursor, 0, "clamps at the top");
    p.on_key(KeyCode::Char('G'));
    assert_eq!(p.cursor, last);
    p.on_key(KeyCode::Char('j'));
    assert_eq!(p.cursor, last, "clamps at the bottom");
    p.on_key(KeyCode::Char('g'));
    assert_eq!(p.cursor, 0);

    assert!(p.on_key(KeyCode::Esc), "esc closes the panel");
}

#[test]
fn x_unsets_the_selected_option() {
    let mut p = panel();
    go_to(&mut p, "--user-agent");
    p.on_key(KeyCode::Enter);
    typed(&mut p, "muxget");
    p.on_key(KeyCode::Enter);
    assert!(p.is_set("--user-agent"));

    p.on_key(KeyCode::Char('x'));
    assert!(!p.is_set("--user-agent"));
}

#[test]
fn a_flag_that_carries_its_value_survives_a_round_trip() {
    let mut p = panel();
    go_to(&mut p, "--seed-time=0");
    p.on_key(KeyCode::Enter);
    assert!(p.is_set("--seed-time=0"));

    // What the file holds, and what it looks like when read back.
    let text = args::render(&p.pairs);
    assert_eq!(text, "--seed-time=0");
    let mut back = panel();
    back.pairs = args::to_pairs(&args::parse(&text));
    assert!(back.is_set("--seed-time=0"), "still ticked after a reload");
    assert!(back.unknown().is_empty(), "and not mistaken for a stray flag");

    // Toggling it off removes it rather than leaving a second copy behind.
    back.cursor = p.cursor;
    back.on_key(KeyCode::Enter);
    assert!(!back.is_set("--seed-time=0"));
    assert!(back.pairs.is_empty());

    // A hand-set value the spec does not pin reads as unticked, not as this.
    let mut other = panel();
    other.pairs = args::to_pairs(&args::parse("--seed-time=120"));
    assert!(!other.is_set("--seed-time=0"));
}
