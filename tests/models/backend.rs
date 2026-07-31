use muxget::models::pick;

#[test]
fn routes_urls_to_the_right_backend() {
    for url in [
        "magnet:?xt=urn:btih:abc",
        "https://example.com/linux.iso",
        "https://example.com/a.tar.gz?token=1",
        "ftp://example.com/file",
    ] {
        assert_eq!(pick(url).unwrap().name(), "aria2c", "{url}");
    }

    for url in [
        "https://youtube.com/watch?v=abc",
        "https://example.com/some/page",
    ] {
        assert_eq!(pick(url).unwrap().name(), "yt-dlp", "{url}");
    }

    assert!(pick("not a url").is_none());
}

#[test]
fn aria2_exit_codes_read_as_english() {
    use muxget::models::backend::Backend;

    let aria2 = muxget::models::aria2::Aria2;
    assert_eq!(aria2.reason(13), "the file already exists");
    assert_eq!(aria2.reason(9), "not enough disk space");
    // Undocumented codes still say something.
    assert_eq!(aria2.reason(99), "exit 99");
    // yt-dlp documents nothing, so it keeps the default.
    assert_eq!(muxget::models::ytdlp::YtDlp.reason(1), "exit 1");
}
