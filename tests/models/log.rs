use muxget::models::log::{self, Entry, Level, DEFAULT_FORMAT};

#[test]
fn levels_order_from_quietest_to_loudest() {
    assert!(Level::Debug < Level::Info, "debug is below info");
    assert!(Level::Warn < Level::Error, "error is the loudest");
}

#[test]
fn level_names_are_read_however_they_are_typed() {
    assert_eq!(Level::named("DEBUG"), Some(Level::Debug));
    assert_eq!(Level::named(" warning "), Some(Level::Warn));
    assert_eq!(Level::named("loud"), None, "not a level");
}

#[test]
fn a_written_line_follows_the_template() {
    let entry = Entry { at: "01:02:03".into(), level: Level::Warn, text: "slow".into() };
    let line = log::render(&entry, "{time} {level}: {text}");
    assert_eq!(line, "01:02:03 warn: slow");
    // The default keeps the date, which the tab does not show.
    assert!(log::render(&entry, DEFAULT_FORMAT).ends_with(" 01:02:03 warn slow"), "date first");
}

#[test]
fn entries_are_kept_in_order_and_clear_together() {
    log::clear();
    log::warn("second");
    log::error("third");
    let texts: Vec<String> = log::entries().iter().map(|e| e.text.clone()).collect();
    assert_eq!(texts, ["second", "third"], "oldest first");
    // Below the default level, so it is not recorded at all.
    log::debug("quiet");
    assert_eq!(log::entries().len(), 2, "debug is filtered out by default");
    log::clear();
    assert!(log::entries().is_empty());
}
