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
fn the_quality_choice_cycles_through_its_presets() {
    use muxget::models::option::QUALITY;

    let mut p = Options { backend: "yt-dlp", cursor: 0, pairs: Vec::new(), editing: None };
    go_to(&mut p, "--format");

    // Unset lands on the first preset, then walks the list and wraps.
    p.on_key(KeyCode::Char(' '));
    assert_eq!(p.value("--format"), Some(QUALITY[0].value));
    p.on_key(KeyCode::Char(' '));
    assert_eq!(p.value("--format"), Some(QUALITY[1].value));
    assert_eq!(p.preset("--format", QUALITY).map(|q| q.label), Some("1080p"));
    for _ in 1..QUALITY.len() {
        p.on_key(KeyCode::Char(' '));
    }
    assert_eq!(p.value("--format"), Some(QUALITY[0].value), "the list wraps");

    // A hand-written selector is kept until the user cycles past it.
    p.pairs = vec![("--format".into(), "bestvideo".into())];
    assert!(p.preset("--format", QUALITY).is_none(), "not one of ours");
    p.on_key(KeyCode::Char('x'));
    assert_eq!(p.value("--format"), None, "x clears it like any other flag");

    // Short forms are what yt-dlp cannot read; the specs use long ones.
    assert!(p.specs().iter().all(|s| !s.flag.starts_with("-f") && s.flag != "-o"));
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

mod settings {
    use crossterm::event::KeyCode;
    use muxget::controllers::options::{Action, Settings, TABS};

    fn panel() -> Settings {
        open(0, "aria2c")
    }

    /// A throwaway config dir: the panel reads and writes real files.
    fn open(tab: usize, backend: &'static str) -> Settings {
        std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join("muxget-tests"));
        let mut panel = Settings::open(tab, backend, Vec::new());
        panel.options.pairs.clear();
        panel
    }

    #[test]
    fn tabs_cycle_and_reset_the_cursor() {
        let mut p = panel();
        assert_eq!(TABS[p.tab], "general");

        p.on_key(KeyCode::Down);
        assert_eq!(p.cursor, 1);
        p.on_key(KeyCode::Tab);
        assert_eq!(TABS[p.tab], "backends");
        assert_eq!(p.cursor, 0, "each tab starts at the top");

        p.on_key(KeyCode::Tab);
        assert_eq!(TABS[p.tab], "crawler");
        p.on_key(KeyCode::Tab);
        assert_eq!(TABS[p.tab], "categories");
        p.on_key(KeyCode::Tab);
        assert_eq!(TABS[p.tab], "general", "wraps around");
        p.on_key(KeyCode::BackTab);
        assert_eq!(TABS[p.tab], "categories", "and backwards");
    }

    #[test]
    fn general_rows_hand_their_action_to_the_app() {
        let mut p = panel();
        assert_eq!(p.on_key(KeyCode::Enter), Action::NextTheme);
        p.on_key(KeyCode::Down);
        assert_eq!(p.on_key(KeyCode::Enter), Action::EditDir);
        p.on_key(KeyCode::Down);
        assert_eq!(p.on_key(KeyCode::Enter), Action::ToggleNerd);
        p.on_key(KeyCode::Down);
        assert_eq!(p.on_key(KeyCode::Enter), Action::ToggleConfirmPlaylist);
        // The list ends rather than wrapping onto nothing.
        p.on_key(KeyCode::Down);
        assert_eq!(p.cursor, 3);
        assert_eq!(p.on_key(KeyCode::Esc), Action::Close);
    }

    #[test]
    fn the_backends_tab_edits_one_backend_at_a_time() {
        let mut p = open(1, "aria2c");
        assert_eq!(p.options.backend, "aria2c");

        // `b` walks to the next backend, `Tab` still changes tab.
        p.on_key(KeyCode::Char('b'));
        assert_eq!(p.options.backend, "yt-dlp");
        p.on_key(KeyCode::Char('b'));
        assert_eq!(p.options.backend, "wget");
        p.on_key(KeyCode::Char('b'));
        assert_eq!(p.options.backend, "aria2c");

        // Toggling reaches the form under the cursor.
        let flag = "--check-integrity";
        p.cursor = p.options.specs().iter().position(|s| s.flag == flag).unwrap();
        p.on_key(KeyCode::Enter);
        assert!(p.options.is_set(flag));
    }

    #[test]
    fn a_value_being_typed_keeps_the_keyboard() {
        let mut p = open(1, "aria2c");
        p.cursor = p.options.specs().iter().position(|s| s.flag == "--split").unwrap();
        p.on_key(KeyCode::Enter);
        assert!(p.options.editing.is_some());

        // `l` would change tab; while editing it is just a character.
        p.on_key(KeyCode::Char('l'));
        assert_eq!(TABS[p.tab], "backends");
        assert_eq!(p.options.editing.as_deref(), Some("l"));
    }
}

#[cfg(test)]
mod categories {
    use crossterm::event::KeyCode;
    use muxget::controllers::options::{Settings, RULE_ROWS};
    use muxget::models::rule::{self, Rule};

    fn panel() -> Settings {
        std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join("muxget-tests-rules"));
        // Tab 3 is categories.
        Settings::open(3, "aria2c", Vec::new())
    }

    fn typed(p: &mut Settings, keys: &str) {
        for c in keys.chars() {
            p.on_key(KeyCode::Char(c));
        }
    }

    #[test]
    fn a_rule_can_be_built_edited_and_removed_from_the_panel() {
        let mut p = panel();
        assert_eq!(p.rows(), 0, "nothing to walk before the first rule");

        // `n` adds a rule and puts the cursor on it.
        p.on_key(KeyCode::Char('n'));
        assert_eq!(p.rules.len(), 1);
        assert_eq!(p.rows(), RULE_ROWS, "a header row plus one per field");

        // Down onto `extensions`, Enter to type into it.
        p.on_key(KeyCode::Down);
        p.on_key(KeyCode::Enter);
        typed(&mut p, ".MP4, mkv");
        p.on_key(KeyCode::Enter);
        assert_eq!(p.rules[0].extensions, ["mp4", "mkv"], "dots and case are cleaned up");

        // The destination fields are plain text.
        p.cursor = 4; // queue
        p.on_key(KeyCode::Enter);
        typed(&mut p, "video");
        p.on_key(KeyCode::Enter);
        assert_eq!(p.rules[0].queue.as_deref(), Some("video"));

        // Esc abandons what was being typed.
        p.on_key(KeyCode::Enter);
        typed(&mut p, "junk");
        p.on_key(KeyCode::Esc);
        assert_eq!(p.rules[0].queue.as_deref(), Some("video"), "unchanged");

        // `x` clears a field, and on the header row deletes the whole rule.
        p.on_key(KeyCode::Char('x'));
        assert_eq!(p.rules[0].queue, None);
        p.cursor = 0;
        p.on_key(KeyCode::Char('x'));
        assert!(p.rules.is_empty());
    }

    #[test]
    fn rules_round_trip_through_the_file_the_panel_writes() {
        let mut rule = Rule::default();
        rule.set(0, "mp4, mkv");
        rule.set(1, "YouTube.com");
        rule.set(2, "500M");
        rule.set(3, "video");
        rule.set(4, "/tmp/video");
        rule.set(5, "yt-dlp");

        let back = rule::parse(&rule::render(&[rule.clone()]));
        assert_eq!(back, vec![rule], "what the panel writes is what it reads");

        // A rule that decides nothing is not a rule; parse drops it.
        let mut empty = Rule::default();
        empty.set(0, "iso");
        assert!(rule::parse(&rule::render(&[empty])).is_empty());
    }
}
