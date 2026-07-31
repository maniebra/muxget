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
            path: None,
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
fn add_dialog_captures_typing_until_escape() {
    let mut app = app_with(&[]);

    app.on_key(KeyCode::Char('a'));
    typed(&mut app, "htp");
    app.on_key(KeyCode::Backspace);
    assert_eq!(app.dialog, Some(Dialog::Add("ht".into())));

    // While a dialog is open, `q` types instead of quitting.
    assert!(!app.on_key(KeyCode::Char('q')));
    assert_eq!(app.dialog, Some(Dialog::Add("htq".into())));

    app.on_key(KeyCode::Esc);
    assert_eq!(app.dialog, None);
    assert!(app.downloads.is_empty());
    assert!(app.on_key(KeyCode::Char('q')), "quits once the dialog is closed");
}

#[test]
fn edit_dialog_prefills_the_selected_url() {
    let mut app = app_with(&[Status::Running, Status::Done]);
    app.selected = 1;

    app.on_key(KeyCode::Char('e'));
    assert_eq!(
        app.dialog,
        Some(Dialog::Edit(1, "https://example.com/1.iso".into()))
    );

    app.on_key(KeyCode::Esc);
    assert_eq!(app.downloads.len(), 2, "escaping changes nothing");
}

#[test]
fn delete_dialog_confirms_before_removing() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);
    app.selected = 1;

    app.on_key(KeyCode::Char('d'));
    assert_eq!(app.dialog, Some(Dialog::Delete(1)));

    app.on_key(KeyCode::Char('n'));
    assert_eq!(app.downloads.len(), 3, "declining keeps the row");

    app.on_key(KeyCode::Char('d'));
    app.on_key(KeyCode::Char('y'));
    assert_eq!(app.downloads.len(), 2);
    assert_eq!(app.downloads[1].id, 2, "the right row was removed");
    assert!(app.visible().contains(&app.selected), "selection stays valid");
}

#[test]
fn key_sequences_show_a_menu_and_swallow_the_next_key() {
    let mut app = app_with(&[]);

    app.on_key(KeyCode::Char('g'));
    assert_eq!(app.pending, Some('g'), "menu is up, waiting for the second key");

    // The second key is consumed by the sequence, never by the normal keymap.
    app.on_key(KeyCode::Char('n'));
    assert_eq!(app.pending, None);
    assert_eq!(app.dialog, Some(Dialog::QueueNew(String::new())));
    app.on_key(KeyCode::Esc);

    // An unbound second key cancels quietly.
    typed(&mut app, "gz");
    assert_eq!(app.pending, None);
    assert_eq!(app.dialog, None);

    // Sequence navigation.
    app.add_queue("media");
    typed(&mut app, "gk");
    assert_eq!(app.queue().name, "default");
    typed(&mut app, "gj");
    assert_eq!(app.queue().name, "media");

    typed(&mut app, "g+");
    assert_eq!(app.queue().max_active, 4);
    typed(&mut app, "g-");
    assert_eq!(app.queue().max_active, 3);
}

#[test]
fn zz_quits_but_a_lone_z_does_not() {
    let mut app = app_with(&[]);
    assert!(!app.on_key(KeyCode::Char('Z')));
    assert!(app.on_key(KeyCode::Char('Z')));

    let mut app = app_with(&[]);
    app.on_key(KeyCode::Char('Z'));
    assert!(!app.on_key(KeyCode::Char('x')), "Zx is not a quit");
}
