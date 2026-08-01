use muxget::models::download::{Download, Overrides, Status};
use muxget::views::ui::name_of;

fn row(url: &str) -> Download {
    Download {
        id: 0,
        queue: 0,
        url: url.into(),
        backend: "yt-dlp",
        over: Overrides::default(),
        status: Status::Queued,
        progress: Default::default(),
        path: None,
        pid: None,
        tries: 0,
    }
}

#[test]
fn a_name_identifies_the_item() {
    // Only the query tells two of these apart.
    assert_eq!(
        name_of(&row("https://www.youtube.com/watch?v=SylFhDMHDnQ")),
        "watch?v=SylFhDMHDnQ"
    );
    // A file url keeps its filename, query or not.
    assert_eq!(name_of(&row("https://example.com/a/arch.iso")), "arch.iso");
    assert_eq!(name_of(&row("https://example.com/arch.iso?token=1")), "arch.iso");
    assert_eq!(name_of(&row("https://example.com/dir/")), "dir");

    // What the backend wrote wins over anything guessed.
    let mut d = row("https://www.youtube.com/watch?v=SylFhDMHDnQ");
    d.over.name = "asked-for.mkv".into();
    assert_eq!(name_of(&d), "asked-for.mkv");
    d.path = Some("/tmp/dl/real name.mkv".into());
    assert_eq!(name_of(&d), "real name.mkv");
}

#[test]
fn a_magnet_shows_its_display_name() {
    let d = row("magnet:?xt=urn:btih:abc123&dn=Some+Release+2024&tr=udp://x");
    assert_eq!(name_of(&d), "Some Release 2024");
    let real = "magnet:?xt=urn:btih:11EC998CE2818DCF19A8B0336381BCCE5EE209CA\
&dn=Family.Guy.S24E09.1080p.WEB.h264-EDITH&tr=http%3A%2F%2Fp4p.arenabg.com";
    assert_eq!(name_of(&row(real)), "Family.Guy.S24E09.1080p.WEB.h264-EDITH");

    // Without a name there is nothing better than the url itself.
    assert_eq!(
        name_of(&row("magnet:?xt=urn:btih:abc123")),
        "magnet:?xt=urn:btih:abc123"
    );
}

#[test]
fn the_list_shows_the_total_size() {
    use muxget::controllers::app::App;
    use muxget::models::queue::{Queue, DEFAULT};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join("muxget-tests-ui"));
    let mut app = App::with_queues(".".into(), vec![Queue::new(DEFAULT, "default", 3)]);
    let mut d = row("https://example.com/arch.iso");
    d.status = Status::Running;
    d.progress.total = "1.4GiB".into();
    d.progress.percent = 42.0;
    app.downloads.push(d);

    let mut term = Terminal::new(TestBackend::new(120, 12)).unwrap();
    term.draw(|f| muxget::views::ui::draw(f, &app)).unwrap();
    let screen = term.backend().buffer().content().iter().map(|c| c.symbol()).collect::<String>();

    assert!(screen.contains("size"), "the column is headed");
    assert!(screen.contains("1.4GiB"), "and carries the total: {screen}");
}
