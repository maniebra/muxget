use crossterm::event::KeyCode;
use muxget::controllers::app::App;
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
fn the_default_queue_cannot_be_deleted() {
    let mut app = app_with(&[]);
    app.delete_queue(0);
    assert_eq!(app.queues.len(), 1);
    assert_eq!(app.queue().name, "default");
}

#[test]
fn slot_count_is_clamped() {
    let mut app = app_with(&[]);
    app.set_max_active(0);
    assert_eq!(app.queue().max_active, 1, "never zero, or nothing would ever run");
    app.set_max_active(999);
    assert_eq!(app.queue().max_active, 16);
}
