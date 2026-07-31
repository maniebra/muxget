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
