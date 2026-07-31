use muxget::controllers::app::App;
use muxget::controllers::downloads::Filter;
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
