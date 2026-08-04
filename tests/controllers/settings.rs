use crossterm::event::KeyCode;
use muxget::controllers::app::App;
use muxget::controllers::keys::Dialog;
use muxget::models::download::{Download, Status};
use muxget::models::queue::{Queue, DEFAULT};

fn app_with(statuses: &[Status]) -> App {
    // Explicit queues and a throwaway config dir: no dependence on, and no
    // damage to, whatever the user has saved.
    std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join("muxget-tests"));
    let mut app = App::with_queues(".".into(), vec![Queue::new(DEFAULT, "default", 3)]);
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
            over: Default::default(),
        path: None,
        pid: None,
        tries: 0,
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
fn the_settings_panel_changes_the_theme_and_the_directory() {
    let mut app = app_with(&[]);
    let first = app.theme.name.clone();

    // `s` opens the panel itself; there is no menu in front of it.
    app.on_key(KeyCode::Char('s'));
    assert_eq!(app.pending, None);
    assert!(app.settings.is_some());

    app.on_key(KeyCode::Enter);
    assert_ne!(app.theme.name, first, "the theme row moves forward");
    app.on_key(KeyCode::Char('T'));
    assert_eq!(app.theme.name, first, "and back");

    // The directory row hands over to the path dialog, closing the panel.
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Enter);
    assert!(app.settings.is_none());
    assert_eq!(app.dialog, Some(Dialog::SetDir(".".into())), "prefilled");

    typed(&mut app, "/definitely/not/here");
    app.on_key(KeyCode::Enter);
    assert_eq!(app.dir.display().to_string(), ".", "bad path is refused");

    // Nerd icons are the third row.
    app.on_key(KeyCode::Char('s'));
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Enter);
    assert!(app.nerd);

    app.on_key(KeyCode::Esc);
    assert!(app.settings.is_none(), "esc closes the panel");
    // `t` alone still does nothing.
    app.on_key(KeyCode::Char('t'));
    assert_eq!(app.theme.name, first);
}

/// A channel is routed on its own url before it is listed. Its entries are
/// `watch?v=…` links that match no rule written about a channel, so a rule
/// that names one has to be applied here or never.
#[test]
fn syncing_routes_the_channel_by_its_own_url() {
    use muxget::models::channel::{self, Channel};
    use muxget::models::rule::Rule;

    // The same throwaway config dir `app_with` uses: the two tests in this
    // file run at once and both set the variable, so pointing them at one
    // place is what keeps either from reading the other's.
    let mut app = app_with(&[]);
    channel::save(&[Channel {
        url: "https://www.youtube.com/@veritasium/videos".into(),
        // Already synced, so the listing is a date range rather than the lot.
        last_sync: "20260601".into(),
    }])
    .expect("temp dir is writable");

    let mut rule = Rule::default();
    rule.set(2, "youtube.com/@*");
    rule.set(4, "$1");
    rule.set(5, "/tmp/muxget-$1");
    app.rules = vec![rule];

    app.sync_channels(None);

    // The queue the pattern's capture named, created because the rule asked
    // for it. yt-dlp itself runs off-thread and is not needed to see this.
    let queue = app
        .queues
        .iter()
        .find(|q| q.name == "veritasium")
        .expect("routed into the queue the rule names");
    assert_ne!(queue.id, muxget::models::queue::DEFAULT);

    // And the sync moved on, so the same videos are not listed twice.
    assert_ne!(channel::load()[0].last_sync, "20260601");
}
