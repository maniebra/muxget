use crossterm::event::KeyCode;
use muxget::controllers::app::App;
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

fn running_row(id: usize) -> Download {
    Download {
        id,
        queue: DEFAULT,
        url: format!("https://example.com/{id}.iso"),
        backend: "aria2c",
        status: Status::Running,
        progress: Default::default(),
        path: None,
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

#[test]
fn pausing_a_queue_freezes_it_and_starts_nothing() {
    let mut app = app_with(&[Status::Running, Status::Queued]);
    app.queues[0].max_active = 3;

    app.toggle_queue_pause();
    assert!(app.queues[0].paused);
    assert_eq!(app.downloads[0].status, Status::Paused);

    // A free slot must not start the queued row while the queue is paused.
    app.pump();
    assert_eq!(app.downloads[1].status, Status::Queued);

    app.toggle_queue_pause();
    assert!(!app.queues[0].paused);
    assert_eq!(app.downloads[0].status, Status::Running, "rows resume with it");
}

#[test]
fn pause_all_covers_every_queue_and_resume_clears_a_mixed_state() {
    let mut app = app_with(&[Status::Running]);
    app.add_queue("media");
    let media = app.queue().id;
    app.downloads.push(Download { queue: media, ..running_row(7) });

    app.toggle_all_pause();
    assert!(app.queues.iter().all(|q| q.paused));
    assert!(app.downloads.iter().all(|d| d.status == Status::Paused));

    // One queue resumed by hand leaves a mixed state; `P` resolves it to all-on.
    app.toggle_queue_pause();
    assert!(app.queues.iter().any(|q| q.paused));

    app.toggle_all_pause();
    assert!(app.queues.iter().all(|q| !q.paused), "resuming wins over pausing");
    assert!(app.downloads.iter().all(|d| d.status == Status::Running));
}

#[test]
fn a_paused_queue_does_not_block_another_queue() {
    let mut app = app_with(&[Status::Queued]);
    app.add_queue("media");
    let media = app.queue().id;
    app.downloads.push(Download { queue: media, ..running_row(7) });

    app.current = 0;
    app.toggle_queue_pause();

    assert!(app.queues[0].paused);
    assert!(!app.queues[1].paused, "the other queue keeps running");
    assert_eq!(app.active_in(media), 1);
    assert_eq!(app.downloads[0].status, Status::Queued, "stays parked");
}

#[test]
fn a_schedule_pauses_outside_its_window_and_resumes_inside() {
    let mut app = app_with(&[Status::Queued]);
    app.set_schedule(0, "22:00-06:00");
    let q = &app.queues[0];
    assert_eq!(q.schedule, Some((22 * 60, 6 * 60)));
    assert!(q.open_at(23 * 60), "inside, past midnight window");
    assert!(q.open_at(5 * 60), "inside, after midnight");
    assert!(!q.open_at(12 * 60), "outside");

    app.set_schedule(0, "09:00-17:00");
    let q = &app.queues[0];
    assert!(q.open_at(12 * 60) && !q.open_at(20 * 60), "plain window");

    // Junk and empty both clear it rather than pausing the queue forever.
    app.set_schedule(0, "nonsense");
    assert_eq!(app.queues[0].schedule, None);
    assert!(app.queues[0].open_at(0), "no window means always open");
}
