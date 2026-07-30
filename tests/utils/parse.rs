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
fn splits_on_carriage_returns_and_newlines() {
    let mut got = Vec::new();
    parse::for_each_line(&b"a\rb\nc\r\n\r\nd"[..], |l| got.push(l.to_string()));
    assert_eq!(got, ["a", "b", "c", "d"]);
}
