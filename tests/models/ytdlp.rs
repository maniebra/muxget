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
fn a_listed_line_splits_into_url_date_and_title() {
    use muxget::models::ytdlp::{entry, Entry};

    assert_eq!(
        entry("https://y.com/watch?v=a\t20220802\tSome Title"),
        Some(Entry {
            url: "https://y.com/watch?v=a".into(),
            date: "20220802".into(),
            title: "Some Title".into(),
        })
    );
    // yt-dlp prints NA for what it has no value for; a site with no dates in
    // its index leaves the entry usable without one.
    let none = entry("https://y.com/watch?v=b\tNA\tNo Date").unwrap();
    assert_eq!((none.date.as_str(), none.title.as_str()), ("", "No Date"));
    // An old yt-dlp, or an entry with no title, still gives a usable url.
    assert_eq!(entry("https://y.com/watch?v=c").unwrap().url, "https://y.com/watch?v=c");
    assert_eq!(entry("[youtube:tab] Extracting URL"), None, "log noise is not an entry");
}

#[test]
fn dates_come_with_the_listing_and_only_fall_back_when_they_cannot() {
    use muxget::models::ytdlp::{date, dated_list_command, is_plain_date, list_command, DateRange};

    // The fast pass: one request for the whole playlist, dates included.
    // `approximate_date` is what puts them in a flat listing at all.
    let fast = format!("{:?}", list_command("https://y.com/playlist?list=x"));
    assert!(fast.contains("--flat-playlist"));
    assert!(fast.contains("approximate_date"), "no dates without it");
    assert!(fast.contains("upload_date"), "and they are printed per entry");

    // Dashes and slashes are for people; yt-dlp wants the digits.
    assert_eq!(date("2020-01-01"), "20200101");
    assert_eq!(date("2023/12/31"), "20231231");
    assert_eq!(date(""), "", "an empty end stays open");
    // Its own shorthand passes through untouched, and is not comparable here.
    assert_eq!(date("now-6months"), "now-6months");
    assert!(is_plain_date("20200101") && !is_plain_date("now-6months") && !is_plain_date("2020"));

    // The slow pass exists for what the fast one cannot answer, and hands the
    // filtering to yt-dlp.
    let both = DateRange { after: date("2020-01-01"), before: date("2023-12-31") };
    let slow = format!("{:?}", dated_list_command("https://y.com/playlist?list=x", &both));
    assert!(!slow.contains("--flat-playlist"), "exact dates need the full extraction");
    assert!(slow.contains("--dateafter") && slow.contains("20200101"));
    assert!(slow.contains("--datebefore") && slow.contains("20231231"));
    assert!(slow.contains("webpage_url"), "the page, not the media stream");

    // One end alone leaves the other open.
    let from = DateRange { after: date("2020-01-01"), ..Default::default() };
    let slow = format!("{:?}", dated_list_command("https://y.com/x", &from));
    assert!(slow.contains("--dateafter") && !slow.contains("--datebefore"));
}
