use muxget::models::queue::Queue;
use muxget::models::download::{Download, Status};
use muxget::models::state::State;

#[test]
fn round_trips_directory_and_queues() {
    let queues = [Queue::new(0, "default", 3), Queue::new(1, "media", 6)];
    let text = State::render(std::path::Path::new("/tmp/dl"), &queues, &[], false);

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
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[q], &[], false));
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
        over: Default::default(),
        path: None,
        pid: None,
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
        false,
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
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[], &downloads, false));
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
    let back = State::parse(&State::render(std::path::Path::new("/tmp"), &[q], &[], false));
    assert_eq!(back.queues[0].schedule, Some((22 * 60, 6 * 60)));

    // Files written before schedules existed still load.
    let old = State::parse("queue = default|4\n");
    assert_eq!(old.queues[0].max_active, 4);
    assert_eq!(old.queues[0].schedule, None);
}

#[test]
fn per_item_settings_round_trip_and_stay_optional() {
    use muxget::models::download::Overrides;

    let mut d = row(0, Status::Queued, "https://example.com/a.iso");
    d.over = Overrides {
        dir: "/tmp/here".into(),
        name: "mine.iso".into(),
        rate: "2M".into(),
        user: "me".into(),
        backend: "aria2c".into(),
        pass: "hunter2".into(),
    };
    let plain = row(0, Status::Done, "https://example.com/b.iso");
    let back = State::parse(&State::render(
        std::path::Path::new("/tmp"),
        &[],
        &[d, plain],
        false,
    ));

    assert_eq!(back.downloads[0].over.dir, "/tmp/here");
    assert_eq!(back.downloads[0].over.rate, "2M");
    assert_eq!(back.downloads[0].over.user, "me");
    assert_eq!(back.downloads[0].over.backend, "aria2c");
    assert_eq!(back.downloads[0].over.pass, "", "a password is never stored");
    assert!(back.downloads[1].over.is_empty(), "no line written when unset");

    // A stray `over` with no download above it is ignored, not a panic.
    assert!(State::parse("over = /tmp||1M\n").downloads.is_empty());
}

#[test]
fn the_nerd_font_choice_round_trips_and_defaults_off() {
    let on = State::parse(&State::render(std::path::Path::new("/tmp"), &[], &[], true));
    assert!(on.nerd);
    let off = State::parse(&State::render(std::path::Path::new("/tmp"), &[], &[], false));
    assert!(!off.nerd);
    // A file written before the option existed.
    assert!(!State::parse("dir = /tmp\n").nerd);
}

#[test]
fn a_running_pid_round_trips_so_the_next_run_can_kill_it() {
    let mut d = row(0, Status::Running, "https://example.com/a.iso");
    d.pid = Some(4242);
    let plain = row(0, Status::Done, "https://example.com/b.iso");
    let back = State::parse(&State::render(
        std::path::Path::new("/tmp"),
        &[],
        &[d, plain],
        false,
    ));

    assert_eq!(back.downloads[0].pid, Some(4242));
    assert_eq!(back.downloads[1].pid, None, "no line written when there is none");
    // A stray pid with no download above it is ignored.
    assert!(State::parse("pid = 1\n").downloads.is_empty());
}
