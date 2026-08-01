use muxget::models::ytdlp;
use muxget::utils::args;

#[test]
fn recognises_playlist_and_channel_urls() {
    for url in [
        "https://www.youtube.com/playlist?list=PLabc",
        "https://youtube.com/watch?v=abc&list=PLabc",
        "https://www.youtube.com/@somechannel",
        "https://www.youtube.com/channel/UC123/videos",
    ] {
        assert!(ytdlp::is_playlist(url), "{url}");
    }

    for url in [
        "https://youtu.be/abc123",
        "https://youtube.com/watch?v=abc123",
        "https://example.com/video.mp4",
    ] {
        assert!(!ytdlp::is_playlist(url), "{url}");
    }
}

#[test]
fn no_playlist_option_disables_expansion() {
    let dir = std::env::temp_dir().join(format!("muxget-pl-{}", std::process::id()));
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    let url = "https://www.youtube.com/playlist?list=PLabc";

    args::save("yt-dlp", "").unwrap();
    assert!(ytdlp::expands_playlist(url));

    args::save("yt-dlp", "--no-playlist").unwrap();
    assert!(!ytdlp::expands_playlist(url), "user asked for single videos");

    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_listed_line_splits_into_url_and_title() {
    use muxget::models::ytdlp::entry;
    assert_eq!(
        entry("https://y.com/watch?v=a\tSome Title"),
        Some(("https://y.com/watch?v=a".into(), "Some Title".into()))
    );
    // An old yt-dlp, or an entry with no title, still gives a usable url.
    assert_eq!(entry("https://y.com/watch?v=b"), Some(("https://y.com/watch?v=b".into(), String::new())));
    assert_eq!(entry("[youtube:tab] Extracting URL"), None, "log noise is not an entry");
}

#[test]
fn a_date_range_is_parsed_and_changes_how_the_listing_runs() {
    use muxget::models::ytdlp::{list_command, DateRange};

    let open = DateRange::parse("");
    assert!(open.is_empty());
    // A flat listing is one request for the whole playlist.
    let flat = format!("{:?}", list_command("https://y.com/playlist?list=x", &open));
    assert!(flat.contains("--flat-playlist") && !flat.contains("--dateafter"));

    let range = DateRange::parse("2020-01-01..2023/12/31");
    assert_eq!(range.after, "20200101", "dashes are for people, not yt-dlp");
    assert_eq!(range.before, "20231231");
    assert_eq!(range.typed(), "20200101..20231231", "and it goes back into the field");

    // Dates are only in the full metadata, so the flat pass has to go.
    let dated = format!("{:?}", list_command("https://y.com/playlist?list=x", &range));
    assert!(!dated.contains("--flat-playlist"));
    assert!(dated.contains("--dateafter") && dated.contains("20200101"));
    assert!(dated.contains("--datebefore") && dated.contains("20231231"));
    assert!(dated.contains("webpage_url"), "the page, not the media stream");

    // One open end, and yt-dlp's own shorthand, both survive as typed.
    let after = DateRange::parse("now-6months..");
    assert_eq!((after.after.as_str(), after.before.as_str()), ("now-6months", ""));
    let before = DateRange::parse("..today");
    assert_eq!((before.after.as_str(), before.before.as_str()), ("", "today"));
}
