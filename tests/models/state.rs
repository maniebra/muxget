use muxget::models::queue::Queue;
use muxget::models::download::{Download, Status};
use muxget::models::state::State;

#[test]
fn round_trips_directory_and_queues() {
    let queues = [Queue::new(0, "default", 3), Queue::new(1, "media", 6)];
    let text = State::render(std::path::Path::new("/tmp/dl"), &queues, &[]);

    let back = State::parse(&text);
    assert_eq!(back.dir, Some("/tmp/dl".into()));
    assert_eq!(back.queues.len(), 2);
    assert_eq!(back.queues[0].name, "default");
    assert_eq!(back.queues[1].name, "media");
    assert_eq!(back.queues[1].max_active, 6);
    assert_eq!(back.queues[1].id, 1, "ids follow file order");
}

#[test]
fn missing_or_broken_lines_fall_back_instead_of_failing() {
    let state = State::parse("garbage\nqueue = \nqueue = media|abc\ndir =\n");
    assert_eq!(state.dir, None);
    assert_eq!(state.queues.len(), 1, "only the usable queue line survives");
    assert_eq!(state.queues[0].max_active, 3, "bad slot count falls back");

    assert!(State::default().queues.is_empty());
    assert_eq!(State::default().queues_or_default().len(), 1, "always a default queue");
}

#[test]
fn pause_state_is_not_persisted() {
    let mut q = Queue::new(0, "default", 3);
    q.paused = true;
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[q], &[]));
    assert!(!back.queues[0].paused, "a restart starts unpaused");
}

fn row(queue: usize, status: Status, url: &str) -> Download {
    Download {
        id: 0,
        queue,
        url: url.into(),
        backend: "aria2c",
        status,
        progress: Default::default(),
        path: None,
        child: None,
    }
}

#[test]
fn downloads_round_trip_and_unfinished_ones_come_back_queued() {
    let downloads = [
        row(0, Status::Running, "https://example.com/a.iso"),
        row(1, Status::Paused, "https://example.com/b.iso"),
        row(0, Status::Done, "https://example.com/c.iso"),
        row(0, Status::Failed("exit 1".into()), "https://example.com/d.iso"),
        row(0, Status::Cancelled, "https://example.com/e.iso"),
    ];
    let back = State::parse(&State::render(
        std::path::Path::new("/tmp"),
        &[Queue::new(0, "default", 3), Queue::new(1, "media", 3)],
        &downloads,
    ));

    assert_eq!(back.downloads.len(), 5);
    assert_eq!(back.downloads[0].status, Status::Queued, "running resumes");
    assert_eq!(back.downloads[1].status, Status::Queued, "paused resumes");
    assert_eq!(back.downloads[1].queue, 1, "queue membership is kept");
    assert_eq!(back.downloads[2].status, Status::Done);
    assert!(matches!(back.downloads[3].status, Status::Failed(_)));
    assert_eq!(back.downloads[4].status, Status::Cancelled);
    assert_eq!(back.downloads[4].url, "https://example.com/e.iso");
}

#[test]
fn a_url_containing_a_pipe_survives() {
    let downloads = [row(0, Status::Done, "https://example.com/x?a=1|2")];
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[], &downloads));
    assert_eq!(back.downloads[0].url, "https://example.com/x?a=1|2");
}

#[test]
fn broken_download_lines_are_skipped() {
    let state = State::parse("download = 0\ndownload = 0|done\ndownload = 0|done|\n");
    assert!(state.downloads.is_empty());
}

#[test]
fn a_queue_window_round_trips_and_is_optional() {
    let mut q = muxget::models::queue::Queue::new(0, "night", 3);
    q.schedule = muxget::models::queue::parse_window("22:00-06:00");
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[q], &[]));
    assert_eq!(back.queues[0].schedule, Some((22 * 60, 6 * 60)));

    // Files written before schedules existed still load.
    let old = State::parse("queue = default|4\n");
    assert_eq!(old.queues[0].max_active, 4);
    assert_eq!(old.queues[0].schedule, None);
}
