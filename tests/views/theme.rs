use muxget::views::theme::Theme;
use ratatui::style::Color;

#[test]
fn builtins_load_and_cycle_wraps() {
    let themes = Theme::all();
    assert!(themes.iter().any(|t| t.name == "tokyonight"));
    assert!(themes.iter().any(|t| t.name == "catppuccin"));

    let mut t = Theme::default();
    for _ in 0..themes.len() {
        t = t.next(&themes);
    }
    assert_eq!(t, Theme::default());
}

#[test]
fn config_parses_hex_and_ignores_junk() {
    let t = Theme::from_config(
        "mine",
        r##"
        # comment
        accent = "#ff8800"
        ok = '#00ff00'
        muted = not-a-color
        bogus = "#000000"
        "##,
    );
    assert_eq!(t.name, "mine");
    assert_eq!(t.accent, Color::Rgb(0xff, 0x88, 0x00));
    assert_eq!(t.ok, Color::Rgb(0, 0xff, 0));
    assert_eq!(t.muted, Theme::default().muted, "bad color keeps default");
}

#[test]
fn unknown_name_falls_back() {
    assert_eq!(Theme::named("nope"), Theme::default());
    assert_eq!(Theme::named("NORD").name, "nord");
}
