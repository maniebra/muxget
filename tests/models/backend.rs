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
