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
    // A magnet's metadata is held in memory, not written as a file.
    assert_eq!(
        destination("b85daa|OK  |       0B/s|[MEMORY][METADATA]Family.Guy.S23"),
        None
    );
    // The result table's header is not a finished row.
    assert_eq!(destination("gid   |stat|avg speed  |path/URI"), None);
    assert_eq!(destination("[download]  42.0% of 1.0GiB at 3MiB/s"), None);
}

#[test]
fn aria2_torrent_line_carries_peers_and_upload() {
    use muxget::utils::parse::aria2;

    let p = aria2("[#8a1f2b 12MiB/100MiB(12%) CN:8 SD:3 DL:5.0MiB UL:1.2MiB(30MiB) ETA:17s]")
        .expect("parsed");
    assert_eq!(p.percent, 12.0);
    assert_eq!(p.speed, "5.0MiB");
    assert_eq!(p.upload, "1.2MiB");
    assert_eq!(p.uploaded, "30MiB", "the bracketed session total");
    assert_eq!(p.peers, 8);
    assert_eq!(p.seeders, Some(3));
    assert_eq!(p.leechers(), 5, "peers that are not seeding");
    assert!(p.is_torrent());

    // A plain http download has no SD/UL, so it is not a torrent.
    let p = aria2("[#8a1f2b 12MiB/100MiB(12%) CN:1 DL:5.0MiB ETA:17s]").expect("parsed");
    assert_eq!(p.peers, 1);
    assert_eq!(p.seeders, None);
    assert!(!p.is_torrent());
    assert_eq!(p.upload, "");
    assert_eq!(p.uploaded, "");
}

#[test]
fn a_torrent_without_a_percent_falls_back_to_the_sizes() {
    use muxget::utils::parse::aria2;

    // What aria2 prints while a magnet's metadata is still coming in.
    let p = aria2("[#b85daa 0B/0B CN:0 SD:0 DL:0B]").expect("parsed");
    assert_eq!(p.percent, 0.0);
    assert!(p.is_torrent());

    // Once the size is known but the percent is still absent.
    let p = aria2("[#b85daa 256MiB/1.0GiB CN:32 SD:12 DL:2.5MiB]").expect("parsed");
    assert_eq!(p.percent, 25.0);
    assert_eq!(p.done, "256MiB");
    assert_eq!(p.total, "1.0GiB");

    // Some builds prefix the sizes.
    assert_eq!(
        aria2("[#b85daa SIZE:512MiB/1.0GiB CN:1 SD:1 DL:0B]").unwrap().percent,
        50.0
    );
    // Not a progress line.
    assert!(aria2("FILE: [MEMORY][METADATA]something").is_none());
}

#[test]
fn ytdlp_progress_carries_the_size() {
    use muxget::utils::parse::ytdlp;

    let p = ytdlp("[download]  42.0% of   27.20MiB at    1.80MiB/s ETA 00:15").unwrap();
    assert_eq!(p.total, "27.20MiB");
    assert_eq!(p.done, "11.4MiB", "yt-dlp reports no running total, only a percentage of one");
    assert_eq!((p.percent, p.speed.as_str(), p.eta.as_str()), (42.0, "1.80MiB/s", "00:15"));

    // Fragmented downloads are estimates, and say so with a `~`.
    let p = ytdlp("[download]  50.0% of ~ 100.00MiB at    2.00MiB/s ETA 00:30").unwrap();
    assert_eq!((p.total.as_str(), p.done.as_str()), ("100.00MiB", "50.0MiB"));

    // The line that closes a file has no ETA and joins the elapsed time.
    let p = ytdlp("[download] 100% of   14.59MiB in 00:00:10 at 1.34MiB/s").unwrap();
    assert_eq!((p.percent, p.total.as_str()), (100.0, "14.59MiB"));

    // Before a size is known, the speed must not be mistaken for one.
    let p = ytdlp("[download]   0.0% of Unknown B at  Unknown B/s ETA Unknown").unwrap();
    assert_eq!((p.total.as_str(), p.done.as_str()), ("", ""));
}
