use muxget::models::channel::{parse, render, Channel};
use muxget::utils::{civil, today};

const CHANNELS: &str = r#"
# Two followed channels.
[[channel]]
url = "https://www.youtube.com/@one"
last_sync = "20260601"

[[channel]]
url = "https://www.youtube.com/@two"
# Hand-written the way a person writes a date.
last_sync = "2026-01-31"

# No url, so there is nothing to sync.
[[channel]]
last_sync = "20260101"
"#;

#[test]
fn reads_channels_and_normalises_dates() {
    let channels = parse(CHANNELS);
    assert_eq!(channels.len(), 2);
    assert_eq!(channels[0].url, "https://www.youtube.com/@one");
    assert_eq!(channels[0].last_sync, "20260601");
    assert_eq!(channels[1].last_sync, "20260131");
}

#[test]
fn round_trips_through_the_file() {
    let channels = parse(CHANNELS);
    assert_eq!(parse(&render(&channels)), channels);
}

#[test]
fn a_never_synced_channel_keeps_an_empty_date() {
    let one = Channel { url: "https://x.com/@c".into(), last_sync: String::new() };
    assert_eq!(parse(&render(std::slice::from_ref(&one))), vec![one]);
}

#[test]
fn civil_dates_match_the_calendar() {
    assert_eq!(civil(0), (1970, 1, 1));
    // A leap day, and the day after a century that is not a leap year.
    assert_eq!(civil(59), (1970, 3, 1));
    assert_eq!(civil(11_016), (2000, 2, 29));
    assert_eq!(civil(20_878), (2027, 3, 1));
    assert_eq!(civil(-1), (1969, 12, 31));
}

#[test]
fn today_is_a_date_yt_dlp_reads() {
    let today = today();
    assert_eq!(today.len(), 8);
    assert!(today.chars().all(|c| c.is_ascii_digit()));
    assert!(muxget::models::ytdlp::is_plain_date(&today));
}
