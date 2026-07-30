use crossterm::event::KeyCode;
use muxget::controllers::app::{App, Dialog, Filter};
use muxget::models::download::{Download, Status};

fn app_with(statuses: &[Status]) -> App {
    let mut app = App::new(".".into());
    app.downloads = statuses
        .iter()
        .enumerate()
        .map(|(id, s)| Download {
            id,
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
fn filter_selects_rows_and_keeps_selection_visible() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);

    assert_eq!(app.visible(), [0, 1, 2]);

    app.set_filter(Filter::Done);
    assert_eq!(app.visible(), [1]);
    assert_eq!(app.selected, 1, "selection jumps to a shown row");

    app.set_filter(Filter::Active);
    assert_eq!(app.visible(), [0, 2]);
    assert_eq!(app.selected, 0);
}

#[test]
fn selection_moves_within_the_filtered_rows_only() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);
    app.set_filter(Filter::Active);

    app.move_selection(1);
    assert_eq!(app.selected, 2, "skips the filtered-out row");
    app.move_selection(1);
    assert_eq!(app.selected, 2, "clamps at the end");
    app.move_selection(-5);
    assert_eq!(app.selected, 0, "clamps at the start");
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
fn delete_of_the_last_row_moves_the_selection_back() {
    let mut app = app_with(&[Status::Running, Status::Running]);
    app.selected = 1;
    app.delete(1);
    assert_eq!(app.selected, 0);

    app.delete(0);
    assert_eq!(app.selected, 0, "empty list stays in bounds");
}

#[test]
fn queue_starts_oldest_first_and_only_when_a_slot_is_free() {
    let mut app = app_with(&[Status::Running, Status::Queued, Status::Queued]);
    app.max_active = 1;

    assert_eq!(app.active(), 1);
    assert_eq!(app.next_queued(), Some(1), "oldest queued row is next");

    // Slot taken: nothing may start.
    app.pump();
    assert_eq!(app.downloads[1].status, Status::Queued);

    // Freeing the slot lets exactly one through — the process would spawn here,
    // so check the accounting instead: one slot, one candidate.
    app.downloads[0].status = Status::Done;
    assert_eq!(app.active(), 0);
    assert_eq!(app.next_queued(), Some(1));
}

#[test]
fn cancelling_a_queued_row_removes_it_from_the_queue() {
    let mut app = app_with(&[Status::Queued, Status::Queued]);
    app.max_active = 0; // keep pump from spawning anything

    app.cancel(0);
    assert_eq!(app.downloads[0].status, Status::Cancelled);
    assert_eq!(app.next_queued(), Some(1));
}

#[test]
fn slot_count_is_clamped() {
    let mut app = app_with(&[]);
    app.set_max_active(0);
    assert_eq!(app.max_active, 1, "never zero, or nothing would ever run");
    app.set_max_active(999);
    assert_eq!(app.max_active, 16);
}

#[test]
fn active_filter_shows_queued_rows_too() {
    let mut app = app_with(&[Status::Queued, Status::Running, Status::Done]);
    app.set_filter(Filter::Active);
    assert_eq!(app.visible(), [0, 1]);
}

#[test]
fn failed_filter_covers_cancelled() {
    let app = app_with(&[Status::Cancelled, Status::Failed("exit 1".into()), Status::Done]);
    assert_eq!(app.visible().len(), 3);
    assert!(Filter::Failed.matches(&Status::Cancelled));
    assert!(!Filter::Failed.matches(&Status::Done));
}
