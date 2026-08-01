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

#[test]
fn a_backend_missing_from_path_is_reported() {
    use muxget::utils::{missing_backends, on_path};

    // A PATH of one directory holding one fake backend, so the answer is
    // known: everything else in the registry is missing.
    let dir = std::env::temp_dir().join("muxget-test-path");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("aria2c"), b"#!/bin/sh\n").unwrap();
    std::env::set_var("PATH", &dir);

    assert!(on_path("aria2c"));
    assert!(!on_path("definitely-not-installed"));
    assert_eq!(missing_backends(), ["yt-dlp", "wget"]);

    std::env::set_var("PATH", "");
    assert_eq!(missing_backends(), ["aria2c", "yt-dlp", "wget"], "nothing on an empty PATH");
}
