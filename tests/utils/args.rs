use muxget::utils::args;

#[test]
fn parses_flags_across_lines_and_skips_comments() {
    let text = "
        # split files and limit speed
        --split=8 --max-download-limit=1M

        --header=Referer:https://example.com
    ";
    assert_eq!(
        args::parse(text),
        [
            "--split=8",
            "--max-download-limit=1M",
            "--header=Referer:https://example.com",
        ]
    );
}

#[test]
fn empty_config_means_no_extra_flags() {
    assert!(args::parse("").is_empty());
    assert!(args::parse("# only a comment\n\n").is_empty());
}

#[test]
fn round_trips_through_the_config_file() {
    let dir = std::env::temp_dir().join(format!("muxget-args-{}", std::process::id()));
    std::env::set_var("XDG_CONFIG_HOME", &dir);

    args::save("aria2c", "  --split=16 --file-allocation=none  ").unwrap();
    assert_eq!(args::load("aria2c"), ["--split=16", "--file-allocation=none"]);
    assert!(args::path("aria2c").ends_with("muxget/aria2c.args"));

    // A backend with no file configured contributes nothing.
    assert!(args::load("yt-dlp").is_empty());

    std::fs::remove_dir_all(dir).ok();
}
