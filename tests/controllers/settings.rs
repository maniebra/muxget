use crossterm::event::KeyCode;
use muxget::controllers::app::App;
use muxget::controllers::keys::Dialog;
use muxget::models::download::{Download, Status};

fn app_with(statuses: &[Status]) -> App {
    let mut app = App::new(".".into());
    app.downloads = statuses
        .iter()
        .enumerate()
        .map(|(id, s)| Download {
            id,
            queue: muxget::models::queue::DEFAULT,
            url: format!("https://example.com/{id}.iso"),
            backend: "aria2c",
            status: s.clone(),
            progress: Default::default(),
            child: None,
        })
        .collect();
    app
}


fn typed(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.on_key(KeyCode::Char(c));
    }
}

#[test]
fn settings_menu_cycles_themes_and_edits_the_directory() {
    let mut app = app_with(&[]);
    let first = app.theme.name.clone();

    app.on_key(KeyCode::Char('s'));
    assert_eq!(app.pending, Some('s'));
    app.on_key(KeyCode::Char('t'));
    assert_ne!(app.theme.name, first, "st moves forward");
    typed(&mut app, "sT");
    assert_eq!(app.theme.name, first, "sT moves back");

    // `t` alone no longer touches the theme.
    app.on_key(KeyCode::Char('t'));
    assert_eq!(app.theme.name, first);

    typed(&mut app, "sd");
    assert_eq!(app.dialog, Some(Dialog::SetDir(".".into())), "prefilled");

    app.on_key(KeyCode::Esc);
    typed(&mut app, "sd");
    typed(&mut app, "/definitely/not/here");
    app.on_key(KeyCode::Enter);
    assert_eq!(app.dir.display().to_string(), ".", "bad path is refused");
}
