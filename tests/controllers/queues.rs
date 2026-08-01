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
            over: Default::default(),
        path: None,
        pid: None,
        tries: 0,
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
        over: Default::default(),
        path: None,
        pid: None,
        tries: 0,
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

#[test]
fn queues_reorder_without_moving_their_downloads() {
    let mut app = app_with(&[Status::Queued]);
    app.add_queue("media");
    let media = app.queue().id;
    app.downloads[0].queue = media;

    app.move_queue(-1);
    assert_eq!(app.current, 0, "the moved queue stays selected");
    let names: Vec<&str> = app.queues.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(names, ["media", "default"]);
    assert_eq!(app.downloads[0].queue, media, "membership is by id, not position");

    // The ends do not wrap; there is nowhere further to go.
    app.move_queue(-1);
    assert_eq!(app.queues[0].name, "media");
}

#[test]
fn a_full_spec_parses_every_part() {
    let mut app = app_with(&[]);
    app.set_schedule(0, "22:00-06:00 mon-fri sync=6h retry=3 quota=150MB/4h once shutdown after=notify-send done");
    let q = &app.queues[0];
    assert_eq!(q.schedule, Some((22 * 60, 6 * 60)));
    assert_eq!(q.days, 0b0011111, "mon..fri");
    assert_eq!(q.sync, Some(6 * 60));
    assert_eq!(q.retry, 3);
    assert_eq!(q.quota, Some((150 * 1024 * 1024, 4 * 60)));
    assert!(q.once && q.shutdown);
    assert_eq!(q.after, "notify-send done", "the command keeps its spaces");
}

#[test]
fn weekdays_dates_and_quotas_all_close_the_window() {
    let mut app = app_with(&[]);
    app.set_schedule(0, "09:00-17:00 mon,wed");
    let noon = |wday: u8, date: &str| muxget::models::queue::Now {
        minutes: 12 * 60,
        weekday: wday,
        date: date.to_string(),
    };
    assert!(app.queues[0].open_now(&noon(1, "2026-08-03")), "monday");
    assert!(!app.queues[0].open_now(&noon(2, "2026-08-04")), "tuesday");

    app.set_schedule(0, "on=2026-08-01");
    assert!(app.queues[0].open_now(&noon(6, "2026-08-01")));
    assert!(!app.queues[0].open_now(&noon(7, "2026-08-02")), "another day");

    app.set_schedule(0, "quota=1MB/4h");
    assert!(app.queues[0].open_now(&noon(1, "2026-08-03")), "quota unspent");
    app.queues[0].used = 2 * 1024 * 1024;
    assert!(!app.queues[0].open_now(&noon(1, "2026-08-03")), "quota spent");
}

#[test]
fn a_failure_is_retried_up_to_the_queues_limit() {
    let mut app = app_with(&[Status::Failed("boom".into())]);
    app.set_schedule(0, "retry=2");
    // Nothing can actually start here, so the row is put back by hand.
    for expected in [1, 2] {
        app.retry_failed(0);
        assert_eq!(app.downloads[0].tries, expected);
        app.downloads[0].status = Status::Failed("boom".into());
    }
    app.retry_failed(0);
    assert_eq!(app.downloads[0].tries, 2, "past the limit the failure sticks");
    assert!(matches!(app.downloads[0].status, Status::Failed(_)));
}

#[test]
fn a_quota_is_charged_from_the_reported_speed() {
    let mut app = app_with(&[Status::Running]);
    app.set_schedule(0, "quota=1MB/4h");
    app.downloads[0].progress.speed = "1.0MiB/s".into();
    app.charge_quotas(2.0);
    assert!(app.queues[0].used >= 2 * 1000 * 1000, "two seconds of traffic");
    app.apply_schedules();
    assert!(app.queues[0].paused, "over quota, so parked until the period rolls");
}

#[test]
fn clearing_a_queue_drops_only_its_finished_rows() {
    use muxget::controllers::keys::Dialog;

    let mut app = app_with(&[
        Status::Done,
        Status::Running,
        Status::Failed("exit 1".into()),
        Status::Queued,
        Status::Cancelled,
    ]);
    app.queues[0].paused = true;
    // A row in another queue must not be touched by clearing this one.
    app.queues.push(Queue::new(1, "media", 3));
    app.downloads[4].queue = 1;

    assert_eq!(app.clearable_in(0, false), 2, "done and failed, not the cancelled one elsewhere");

    app.on_key(KeyCode::Char('g'));
    app.on_key(KeyCode::Char('c'));
    assert!(matches!(app.dialog, Some(Dialog::QueueClear(0, false))), "it asks first");
    app.on_key(KeyCode::Char('y'));

    let left: Vec<Status> = app.downloads.iter().map(|d| d.status.clone()).collect();
    assert_eq!(left, [Status::Running, Status::Queued, Status::Cancelled]);
    assert!(app.selected < app.downloads.len(), "the cursor stays in bounds");

    // Nothing finished left here, so the command says so instead of asking.
    app.on_key(KeyCode::Char('g'));
    app.on_key(KeyCode::Char('c'));
    assert!(app.dialog.is_none());
    assert_eq!(app.message, "nothing finished to clear");

    // `C` takes the rest of the queue with it, running rows included.
    app.on_key(KeyCode::Char('C'));
    assert!(matches!(app.dialog, Some(Dialog::QueueClear(0, true))));
    app.on_key(KeyCode::Char('y'));
    let left: Vec<usize> = app.downloads.iter().map(|d| d.queue).collect();
    assert_eq!(left, [1], "only the row in the other queue is left");

    app.on_key(KeyCode::Char('C'));
    assert!(app.dialog.is_none());
    assert_eq!(app.message, "this queue is already empty");
}
