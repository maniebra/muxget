use muxget::utils::parse;

#[test]
fn aria2_progress_line() {
    let p = parse::aria2("[#8a1f2b 12MiB/100MiB(12%) CN:1 DL:5.0MiB ETA:17s]").unwrap();
    assert_eq!(p.percent, 12.0);
    assert_eq!(p.speed, "5.0MiB");
    assert_eq!(p.eta, "17s");
    assert!(parse::aria2("some warning without progress").is_none());
}

#[test]
fn ytdlp_progress_line() {
    let p = parse::ytdlp("[download]  12.3% of   10.00MiB at    5.00MiB/s ETA 00:20").unwrap();
    assert_eq!(p.percent, 12.3);
    assert_eq!(p.speed, "5.00MiB/s");
    assert_eq!(p.eta, "00:20");
    assert!(parse::ytdlp("[info] fetching formats").is_none());
}

#[test]
fn byte_sizes_round_trip() {
    assert_eq!(parse::bytes("5.0MiB/s"), Some(5.0 * 1024.0 * 1024.0));
    assert_eq!(parse::bytes("512KB"), Some(512.0 * 1024.0));
    assert_eq!(parse::bytes("900"), Some(900.0));
    assert_eq!(parse::bytes(""), None);
    assert_eq!(parse::human(5.0 * 1024.0 * 1024.0), "5.0MiB");
    assert_eq!(parse::human(0.0), "0.0B");
}

#[test]
fn splits_on_carriage_returns_and_newlines() {
    let mut got = Vec::new();
    parse::for_each_line(&b"a\rb\nc\r\n\r\nd"[..], |l| got.push(l.to_string()));
    assert_eq!(got, ["a", "b", "c", "d"]);
}

#[test]
fn destination_lines_from_both_backends() {
    use muxget::utils::parse::destination;
    assert_eq!(
        destination("[download] Destination: /tmp/dl/video.mp4").as_deref(),
        Some("/tmp/dl/video.mp4")
    );
    assert_eq!(
        destination("[Merger] Merging formats into \"/tmp/dl/v.mkv\"").as_deref(),
        Some("/tmp/dl/v.mkv")
    );
    assert_eq!(
        destination("[download] /tmp/dl/v.mp4 has already been downloaded").as_deref(),
        Some("/tmp/dl/v.mp4")
    );
    assert_eq!(
        destination("a1b2c3|OK  |   1.2MiB/s|/tmp/dl/arch.iso").as_deref(),
        Some("/tmp/dl/arch.iso")
    );
    // The result table's header is not a finished row.
    assert_eq!(destination("gid   |stat|avg speed  |path/URI"), None);
    assert_eq!(destination("[download]  42.0% of 1.0GiB at 3MiB/s"), None);
}
