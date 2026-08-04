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
    let slow = format!("{:?}", dated_list_command(&["https://y.com/playlist?list=x".to_string()], &both));
    assert!(!slow.contains("--flat-playlist"), "exact dates need the full extraction");
    assert!(slow.contains("--dateafter") && slow.contains("20200101"));
    assert!(slow.contains("--datebefore") && slow.contains("20231231"));
    assert!(slow.contains("webpage_url"), "the page, not the media stream");

    // One end alone leaves the other open.
    let from = DateRange { after: date("2020-01-01"), ..Default::default() };
    let slow = format!("{:?}", dated_list_command(&["https://y.com/x".to_string()], &from));
    assert!(slow.contains("--dateafter") && !slow.contains("--datebefore"));

    // Every url in doubt goes in one invocation, not one process each.
    let many = ["https://y.com/a".to_string(), "https://y.com/b".to_string()];
    let slow = format!("{:?}", dated_list_command(&many, &both));
    assert!(slow.contains("y.com/a") && slow.contains("y.com/b"));
}

/// The whole point of the two passes: only entries near an end of the range
/// cost a request, and the rest are settled by the listing that is free.
#[test]
fn only_entries_near_the_cutoff_need_their_exact_date() {
    use muxget::models::ytdlp::{judge, margin, DateRange, Entry, Verdict};
    use muxget::utils::days_from_civil;

    let today = days_from_civil(2026, 8, 4);
    let at = |y, m, d| Entry {
        url: "https://y.com/v".into(),
        date: format!("{y:04}{m:02}{d:02}"),
        title: String::new(),
    };
    // A sync from a month ago: a few days of slop, not months.
    let since = DateRange { after: "20260701".into(), ..Default::default() };
    assert_eq!(margin(today - days_from_civil(2026, 7, 1)), 5);

    assert_eq!(judge(&at(2026, 8, 1), &since, today), Verdict::Keep);
    assert_eq!(judge(&at(2026, 5, 1), &since, today), Verdict::Drop);
    // Within the slop either way — only the real date can say.
    assert_eq!(judge(&at(2026, 7, 3), &since, today), Verdict::Unsure);
    assert_eq!(judge(&at(2026, 6, 28), &since, today), Verdict::Unsure);

    // An old cutoff is guessed at more loosely, so the band is wider.
    let old = DateRange { after: "20200101".into(), ..Default::default() };
    assert_eq!(judge(&at(2020, 3, 1), &old, today), Verdict::Unsure);
    assert_eq!(judge(&at(2021, 6, 1), &old, today), Verdict::Keep);

    // Both ends, and an entry outside either one.
    let window = DateRange { after: "20240101".into(), before: "20241231".into() };
    assert_eq!(judge(&at(2024, 6, 1), &window, today), Verdict::Keep);
    assert_eq!(judge(&at(2025, 6, 1), &window, today), Verdict::Drop);

    // A listing that gave no date decides nothing; it goes to the exact pass
    // rather than being dropped unseen.
    let undated = Entry { url: "https://y.com/v".into(), ..Default::default() };
    assert_eq!(judge(&undated, &since, today), Verdict::Unsure);
    // With no range at all there is nothing to judge against.
    assert_eq!(judge(&at(1999, 1, 1), &DateRange::default(), today), Verdict::Keep);
}
