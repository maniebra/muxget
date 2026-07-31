use muxget::controllers::app::App;
use muxget::controllers::downloads::Filter;
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

#[test]
fn pause_and_resume_toggle_without_losing_the_row() {
    // No queued rows here: a freed slot would immediately start a real spawn.
    let mut app = app_with(&[Status::Running]);

    app.toggle_pause(0);
    assert_eq!(app.downloads[0].status, Status::Paused);
    assert_eq!(app.downloads.len(), 1, "the row stays");
    assert_eq!(app.active_in(DEFAULT), 0, "a paused download frees its slot");

    app.toggle_pause(0);
    assert_eq!(app.downloads[0].status, Status::Running);
}

#[test]
fn pause_only_applies_to_in_flight_downloads() {
    let mut app = app_with(&[Status::Done, Status::Queued, Status::Cancelled]);
    for at in 0..3 {
        app.toggle_pause(at);
    }
    assert_eq!(app.downloads[0].status, Status::Done);
    assert_eq!(app.downloads[1].status, Status::Queued, "queued is not paused");
    assert_eq!(app.downloads[2].status, Status::Cancelled);
}

#[test]
fn a_paused_download_can_still_be_cancelled_and_counts_as_active() {
    let mut app = app_with(&[Status::Running]);
    app.toggle_pause(0);
    assert_eq!(app.visible(), [0]);

    app.set_filter(Filter::Active);
    assert_eq!(app.visible(), [0], "paused rows show under the active filter");

    app.cancel(0);
    assert_eq!(app.downloads[0].status, Status::Cancelled);
}

#[test]
fn restore_rebuilds_last_sessions_list() {
    use muxget::models::state::SavedDownload;

    let mut app = app_with(&[]);
    app.queues.push(Queue::new(1, "media", 3));
    app.queues[0].paused = true; // keep restored rows from spawning
    app.queues[1].paused = true;

    app.restore(&[
        SavedDownload { over: Default::default(), pid: None, queue: 0, status: Status::Queued, percent: 42.0, url: "https://a.com/x.iso".into() },
        SavedDownload { over: Default::default(), pid: None, queue: 1, status: Status::Done, percent: 0.0, url: "https://b.com/y.iso".into() },
        SavedDownload { over: Default::default(), pid: None, queue: 9, status: Status::Queued, percent: 0.0, url: "https://c.com/z.iso".into() },
        SavedDownload { over: Default::default(), pid: None, queue: 0, status: Status::Queued, percent: 0.0, url: "not a url".into() },
    ]);

    assert_eq!(app.downloads.len(), 3, "the unroutable url is dropped");
    assert_eq!(app.downloads[0].status, Status::Queued);
    assert_eq!(app.downloads[1].queue, 1, "queue membership is kept");
    assert_eq!(app.downloads[2].queue, DEFAULT, "a vanished queue falls back");

    let ids: Vec<usize> = app.downloads.iter().map(|d| d.id).collect();
    assert_eq!(ids, [0, 1, 2], "ids are reassigned, not reused from the file");
    assert!(app.downloads.iter().all(|d| d.pid.is_none()));
    assert_eq!(app.downloads[0].progress.percent, 42.0, "progress survives a restart");
    assert_eq!(app.downloads[1].progress.percent, 100.0, "a done row is full");
}

#[test]
fn remove_with_data_deletes_the_file_and_its_sidecar() {
    let dir = std::env::temp_dir().join("muxget-test-remove-data");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("x.iso");
    std::fs::write(&file, b"data").unwrap();
    std::fs::write(file.with_extension("aria2"), b"meta").unwrap();

    let mut app = app_with(&[Status::Done]);
    app.downloads[0].path = Some(file.clone());
    app.delete_with_data(0);

    assert!(app.downloads.is_empty());
    assert!(!file.exists());
    assert!(!file.with_extension("aria2").exists(), "the sidecar too");

    // A row that never wrote anything still removes.
    let mut app = app_with(&[Status::Queued]);
    app.delete_with_data(0);
    assert!(app.downloads.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn force_restart_is_refused_for_anything_but_a_torrent() {
    let mut app = app_with(&[Status::Queued]);
    app.queues[0].paused = true;

    app.force_restart(0);
    assert_eq!(app.message, "force restart is for torrents");
    assert_eq!(app.downloads[0].status, Status::Queued, "left alone");
    assert!(muxget::models::aria2::is_torrent("magnet:?xt=urn:btih:abc"));
    assert!(muxget::models::aria2::is_torrent("https://x.com/a.TORRENT"));
    assert!(!muxget::models::aria2::is_torrent("https://x.com/a.iso"));
}

#[test]
fn a_download_moves_within_its_queue_and_changes_what_starts_next() {
    let mut app = app_with(&[Status::Queued, Status::Queued, Status::Queued]);
    app.queues[0].paused = true;
    let last = app.downloads[2].id;

    app.selected = 2;
    app.move_download(-1);
    assert_eq!(app.downloads[1].id, last, "moved up one place");
    assert_eq!(app.downloads[app.selected].id, last, "selection follows it");

    app.move_download(-1);
    assert_eq!(app.downloads[0].id, last);
    assert_eq!(app.next_queued(app.queues[0].id), Some(0), "it starts first now");

    // The top and bottom hold.
    app.move_download(-1);
    assert_eq!(app.downloads[0].id, last);
}

#[test]
fn rules_route_new_downloads_by_url_and_by_size() {
    use muxget::models::rule::parse;

    let mut app = app_with(&[]);
    app.queues[0].paused = true;
    app.rules = parse(
        "[[rule]]\nextensions = [\"iso\"]\nqueue = \"large-files\"\ndirectory = \"/tmp/isos\"\n\
         [[rule]]\ndomains = [\"youtube.com\"]\nqueue = \"media\"\nbackend = \"yt-dlp\"\n\
         [[rule]]\nmin_size = \"5G\"\nqueue = \"overnight\"\n",
    );

    app.add("https://example.com/arch.iso");
    let large = queue_id(&app, "large-files");
    assert_eq!(app.downloads[0].queue, large);
    assert_eq!(app.downloads[0].over.dir, "/tmp/isos");

    app.add("https://www.youtube.com/watch?v=abc");
    assert_eq!(app.downloads[1].queue, queue_id(&app, "media"));
    assert_eq!(app.downloads[1].over.backend, "yt-dlp");

    // An unmatched url stays in the queue being viewed.
    app.add("https://example.com/notes.txt");
    assert_eq!(app.downloads[2].queue, app.queues[0].id);

    // Size is only known once the backend reports it.
    let id = app.downloads[2].id;
    app.route_by_size(id, 6.0 * 1024.0 * 1024.0 * 1024.0);
    assert_eq!(app.downloads[2].queue, queue_id(&app, "overnight"));

    // A small one is left where it is.
    app.route_by_size(app.downloads[0].id, 1024.0);
    assert_eq!(app.downloads[0].queue, large);
}

fn queue_id(app: &App, name: &str) -> usize {
    app.queues.iter().find(|q| q.name == name).expect("queue created").id
}
