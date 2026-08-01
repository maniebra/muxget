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
        SavedDownload { over: Default::default(), pid: None, tries: 0, path: None, queue: 0, status: Status::Queued, percent: 42.0, url: "https://a.com/x.iso".into() },
        SavedDownload { over: Default::default(), pid: None, tries: 0, path: None, queue: 1, status: Status::Done, percent: 0.0, url: "https://b.com/y.iso".into() },
        SavedDownload { over: Default::default(), pid: None, tries: 0, path: None, queue: 9, status: Status::Queued, percent: 0.0, url: "https://c.com/z.iso".into() },
        SavedDownload { over: Default::default(), pid: None, tries: 0, path: None, queue: 0, status: Status::Queued, percent: 0.0, url: "not a url".into() },
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

#[test]
fn a_pause_and_its_spent_retries_survive_a_restart() {
    use muxget::models::state::SavedDownload;
    let mut app = app_with(&[]);
    app.restore(&[SavedDownload {
        over: Default::default(),
        pid: None,
        tries: 2,
        path: None,
        queue: 0,
        status: Status::Paused,
        percent: 40.0,
        url: "https://a.com/x.iso".into(),
    }]);
    assert_eq!(app.downloads[0].status, Status::Paused, "still paused");
    assert_eq!(app.downloads[0].tries, 2, "retries already spent are kept");

    // The process it was stopped with is gone, so resuming has to requeue it.
    app.toggle_pause(0);
    assert!(
        matches!(app.downloads[0].status, Status::Queued | Status::Running),
        "resumed from its partial file, not stuck claiming a dead process"
    );
}

#[test]
fn picked_links_are_queued_under_the_paths_their_urls_map_to() {
    use muxget::models::crawl::{Crawl, Found};
    let mut app = app_with(&[]);
    app.dir = "/tmp/dl".into();
    let crawl = Crawl { url: "https://x.com/docs/".into(), ..Default::default() };
    let found = [
        Found { url: "https://x.com/docs/a/manual.pdf".into(), size: Some(10.0) },
        Found { url: "https://x.com/get.php?id=7".into(), size: None },
        Found { url: "https://x.com/docs/skipped.pdf".into(), size: None },
    ];

    app.add_found(&crawl, &found, &[0, 1]);
    assert_eq!(app.downloads.len(), 2, "only the picked links are queued");
    assert_eq!(app.downloads[0].over.dir, "/tmp/dl/x.com/docs/a");
    assert_eq!(app.downloads[0].over.name, "", "the url already names the file");
    assert_eq!(app.downloads[1].over.name, "get@id=7.php", "a query string cannot be a name");

    // Flat mode puts everything in the download directory instead.
    app.downloads.clear();
    app.add_found(&Crawl { flat: true, ..crawl }, &found, &[0]);
    assert_eq!(app.downloads[0].over.dir, "");
}

#[test]
fn manual_retry_requeues_failed_and_cancelled_but_not_done() {
    let mut app = app_with(&[Status::Failed("exit 1".into()), Status::Cancelled, Status::Done]);
    app.queues[0].paused = true; // keep the requeued rows from spawning
    app.downloads[0].tries = 9;
    app.retry(0);
    app.retry(1);
    app.retry(2);
    assert_eq!(app.downloads[0].status, Status::Queued);
    // Past the queue's retry limit still retries by hand, with a fresh budget.
    assert_eq!(app.downloads[0].tries, 0);
    assert_eq!(app.downloads[1].status, Status::Queued);
    assert_eq!(app.downloads[2].status, Status::Done);
}

#[test]
fn a_listed_playlist_queues_only_the_picked_entries() {
    use crossterm::event::KeyCode;
    use muxget::controllers::keys::Dialog;

    let mut app = app_with(&[]);
    app.queues[0].paused = true; // nothing may spawn during the test
    app.listed(muxget::models::ytdlp::Listing {
        url: "https://y.com/playlist?list=x".into(),
        queue: DEFAULT,
        entries: vec![
            ("https://y.com/watch?v=a".into(), "First".into()),
            ("https://y.com/watch?v=b".into(), "Second".into()),
            ("https://y.com/watch?v=c".into(), String::new()),
        ],
        ..Default::default()
    });
    assert!(matches!(app.dialog, Some(Dialog::Playlist(_))), "the picker opens");

    // Everything starts picked; space on the second row drops it.
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Char(' '));
    // A directory typed here applies to every entry queued from this list.
    app.on_key(KeyCode::Char('d'));
    for c in "/tmp".chars() {
        app.on_key(KeyCode::Char(c));
    }
    app.on_key(KeyCode::Enter);
    app.on_key(KeyCode::Enter);

    assert!(app.dialog.is_none(), "Enter closes the picker");
    let urls: Vec<&str> = app.downloads.iter().map(|d| d.url.as_str()).collect();
    assert_eq!(urls, ["https://y.com/watch?v=a", "https://y.com/watch?v=c"]);
    assert!(app.downloads.iter().all(|d| d.over.dir == "/tmp"));
}

#[test]
fn the_word_filter_hides_rows_and_decides_what_is_queued() {
    use crossterm::event::KeyCode;
    use muxget::controllers::keys::Dialog;
    use muxget::models::ytdlp::Listing;

    let mut app = app_with(&[]);
    app.queues[0].paused = true;
    app.listed(Listing {
        url: "https://y.com/playlist?list=x".into(),
        queue: DEFAULT,
        entries: vec![
            ("https://y.com/watch?v=a".into(), "Lecture 1: Sorting".into()),
            ("https://y.com/watch?v=b".into(), "Recitation 1".into()),
            ("https://y.com/watch?v=c".into(), "Lecture 2: Hashing".into()),
        ],
        ..Default::default()
    });

    // `/` filters by word: case is ignored and `-word` excludes.
    app.on_key(KeyCode::Char('/'));
    for c in "lecture -hashing".chars() {
        app.on_key(KeyCode::Char(c));
    }
    app.on_key(KeyCode::Enter);
    let Some(Dialog::Playlist(pick)) = &app.dialog else { panic!("the picker stays open") };
    assert_eq!(pick.shown(), [0], "only the sorting lecture is left");
    assert_eq!(pick.picked, [0], "what the filter reveals is what is picked");

    // Space works on the row on screen, not on the entry underneath it.
    app.on_key(KeyCode::Char(' '));
    let Some(Dialog::Playlist(pick)) = &app.dialog else { panic!() };
    assert!(pick.picked.is_empty());
    app.on_key(KeyCode::Char('a'));

    app.on_key(KeyCode::Enter);
    let urls: Vec<&str> = app.downloads.iter().map(|d| d.url.as_str()).collect();
    assert_eq!(urls, ["https://y.com/watch?v=a"], "hidden rows are never queued");
}

#[test]
fn a_paste_is_previewed_before_anything_is_queued() {
    use crossterm::event::KeyCode;
    use muxget::controllers::keys::Dialog;

    let mut app = app_with(&[]);
    app.queues[0].paused = true;

    app.preview_paste("grab these:\nhttps://a.com/x.iso\nnope\nhttps://a.com/y.iso\n");
    assert!(matches!(app.dialog, Some(Dialog::Paste(..))), "nothing is queued yet");
    assert!(app.downloads.is_empty());

    // Everything starts picked; space drops the row the cursor is on.
    app.on_key(KeyCode::Char(' '));
    app.on_key(KeyCode::Enter);
    let urls: Vec<&str> = app.downloads.iter().map(|d| d.url.as_str()).collect();
    assert_eq!(urls, ["https://a.com/y.iso"]);

    // Esc queues nothing at all.
    app.preview_paste("https://a.com/z.iso");
    app.on_key(KeyCode::Esc);
    assert!(app.dialog.is_none());
    assert_eq!(app.downloads.len(), 1, "the cancelled paste added nothing");

    // Text with no url says so instead of opening an empty preview.
    app.preview_paste("just a note");
    assert!(app.dialog.is_none());
    assert_eq!(app.message, "no urls in the clipboard");
}

#[test]
fn a_rule_pattern_routes_each_channel_to_its_own_directory() {
    use muxget::models::rule::Rule;

    let mut app = app_with(&[]);
    app.queues[0].paused = true;
    let mut channel = Rule::default();
    channel.set(2, "youtube.com/@*"); // pattern
    channel.set(5, "/srv/yt/$1"); // directory
    let mut project = Rule::default();
    project.set(2, "example.com/*/");
    project.set(5, "/srv/$1");
    app.rules = vec![channel, project];

    // A plain url routes on the way in.
    app.add("https://example.com/rust/x.iso");
    app.add("https://other.com/y.iso");
    let dirs: Vec<&str> = app.downloads.iter().map(|d| d.over.dir.as_str()).collect();
    assert_eq!(dirs, ["/srv/rust", ""], "one directory per capture");

    // A channel expands into `watch?v=…` entries, which match no rule written
    // about a channel — so the channel's own routing has to reach them.
    let mut over = Default::default();
    let queue = app.route("https://youtube.com/@mitocw/videos", DEFAULT, &mut over);
    assert_eq!(over.dir, "/srv/yt/mitocw", "decided from the playlist url, not its entries");
    app.enqueue("https://youtube.com/watch?v=abc", queue, over);
    assert_eq!(app.downloads[2].over.dir, "/srv/yt/mitocw");
}
