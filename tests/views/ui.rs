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
        child: None,
    }
}

#[test]
fn a_name_identifies_the_item() {
    // A route plus a query: the query is the only thing that differs.
    assert_eq!(
        name_of(&row("https://www.youtube.com/watch?v=SylFhDMHDnQ")),
        "watch?v=SylFhDMHDnQ"
    );
    // A plain file url keeps its filename, query or not.
    assert_eq!(name_of(&row("https://example.com/a/arch.iso")), "arch.iso");
    assert_eq!(name_of(&row("https://example.com/arch.iso?token=1")), "arch.iso");
    assert_eq!(name_of(&row("https://example.com/dir/")), "dir");

    // What the backend actually wrote wins over anything guessed.
    let mut d = row("https://www.youtube.com/watch?v=SylFhDMHDnQ");
    d.over.name = "asked-for.mkv".into();
    assert_eq!(name_of(&d), "asked-for.mkv");
    d.path = Some("/tmp/dl/real name.mkv".into());
    assert_eq!(name_of(&d), "real name.mkv");
}
