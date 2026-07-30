use crossterm::event::KeyCode;
use muxget::controllers::app::{App, Dialog, Filter};
use muxget::models::download::{Download, Status};
use muxget::models::queue::DEFAULT;

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

fn running_row(id: usize) -> Download {
    Download {
        id,
        queue: DEFAULT,
        url: format!("https://example.com/{id}.iso"),
        backend: "aria2c",
        status: Status::Running,
        progress: Default::default(),
        child: None,
    }
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
    app.queues[0].max_active = 1;

    assert_eq!(app.active_in(DEFAULT), 1);
    assert_eq!(app.next_queued(DEFAULT), Some(1), "oldest queued row is next");

    // Slot taken: nothing may start.
    app.pump();
    assert_eq!(app.downloads[1].status, Status::Queued);

    // Freeing the slot lets exactly one through — the process would spawn here,
    // so check the accounting instead: one slot, one candidate.
    app.downloads[0].status = Status::Done;
    assert_eq!(app.active_in(DEFAULT), 0);
    assert_eq!(app.next_queued(DEFAULT), Some(1));
}

#[test]
fn cancelling_a_queued_row_removes_it_from_the_queue() {
    let mut app = app_with(&[Status::Queued, Status::Queued]);
    app.queues[0].max_active = 1;
    app.downloads.push(running_row(9)); // occupy the only slot so pump spawns nothing

    app.cancel(0);
    assert_eq!(app.downloads[0].status, Status::Cancelled);
    assert_eq!(app.next_queued(DEFAULT), Some(1));
}

#[test]
fn slot_count_is_clamped() {
    let mut app = app_with(&[]);
    app.set_max_active(0);
    assert_eq!(app.queue().max_active, 1, "never zero, or nothing would ever run");
    app.set_max_active(999);
    assert_eq!(app.queue().max_active, 16);
}

#[test]
fn active_filter_shows_queued_rows_too() {
    let mut app = app_with(&[Status::Queued, Status::Running, Status::Done]);
    app.set_filter(Filter::Active);
    assert_eq!(app.visible(), [0, 1]);
}

#[test]
fn queues_are_created_renamed_and_switched() {
    let mut app = app_with(&[]);
    assert_eq!(app.queue().name, "default");

    typed(&mut app, "gn");
    typed(&mut app, "media");
    app.on_key(KeyCode::Enter);
    assert_eq!(app.queues.len(), 2);
    assert_eq!(app.queue().name, "media", "switches to the new queue");

    // Duplicate names are refused.
    app.add_queue("media");
    assert_eq!(app.queues.len(), 2);
    app.add_queue("   ");
    assert_eq!(app.queues.len(), 2);

    app.rename_queue(1, "video");
    assert_eq!(app.queue().name, "video");
    app.rename_queue(1, "default");
    assert_eq!(app.queue().name, "video", "cannot collide with another name");

    app.cycle_queue(1);
    assert_eq!(app.queue().name, "default", "wraps around");
}

#[test]
fn each_queue_has_its_own_slots_and_rows() {
    let mut app = app_with(&[Status::Running, Status::Queued]);
    app.add_queue("media");
    let media = app.queue().id;
    app.downloads.push(Download { queue: media, ..running_row(7) });

    assert_eq!(app.visible(), [2], "only the current queue's rows are listed");
    assert_eq!(app.active_in(DEFAULT), 1);
    assert_eq!(app.active_in(media), 1);
    assert_eq!(app.next_queued(media), None, "the queued row belongs to default");

    app.set_max_active(9);
    assert_eq!(app.queue().max_active, 9);
    assert_eq!(app.queues[0].max_active, 3, "the other queue is untouched");
}

#[test]
fn deleting_a_queue_moves_its_downloads_to_default() {
    let mut app = app_with(&[Status::Running]);
    app.add_queue("media");
    let media = app.queue().id;
    app.downloads.push(Download { queue: media, ..running_row(7) });

    typed(&mut app, "gd");
    app.on_key(KeyCode::Char('y'));

    assert_eq!(app.queues.len(), 1);
    assert_eq!(app.queue().name, "default");
    assert_eq!(app.downloads.len(), 2, "no download was dropped");
    assert!(app.downloads.iter().all(|d| d.queue == DEFAULT));
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

#[test]
fn zz_quits_but_a_lone_z_does_not() {
    let mut app = app_with(&[]);
    assert!(!app.on_key(KeyCode::Char('Z')));
    assert!(app.on_key(KeyCode::Char('Z')));

    let mut app = app_with(&[]);
    app.on_key(KeyCode::Char('Z'));
    assert!(!app.on_key(KeyCode::Char('x')), "Zx is not a quit");
}

#[test]
fn the_default_queue_cannot_be_deleted() {
    let mut app = app_with(&[]);
    app.delete_queue(0);
    assert_eq!(app.queues.len(), 1);
    assert_eq!(app.queue().name, "default");
}

#[test]
fn failed_filter_covers_cancelled() {
    let app = app_with(&[Status::Cancelled, Status::Failed("exit 1".into()), Status::Done]);
    assert_eq!(app.visible().len(), 3);
    assert!(Filter::Failed.matches(&Status::Cancelled));
    assert!(!Filter::Failed.matches(&Status::Done));
}
