use muxget::models::crawl::{collect, local_dir, local_name, wild, Crawl, Found};

fn found(url: &str, size: Option<f64>) -> Found {
    Found { url: url.into(), size }
}

#[test]
fn wget_log_lines_become_a_deduplicated_list() {
    let log = [
        "--2026-07-31 10:00:00--  https://x.com/a.pdf",
        "Length: 1024 (1.0K) [application/pdf]",
        "--2026-07-31 10:00:01--  https://x.com/b.zip",
        "Length: unspecified [text/html]",
        // The same file linked from a second page.
        "--2026-07-31 10:00:02--  https://x.com/a.pdf",
        "Reusing existing connection.",
    ];
    let mut out = Vec::new();
    for line in log {
        collect(line, &mut out);
    }
    assert_eq!(out.len(), 2, "a url found twice is one entry");
    assert_eq!(out[0], found("https://x.com/a.pdf", Some(1024.0)));
    assert_eq!(out[1].size, None, "an unspecified length is not a size");
}

#[test]
fn a_url_the_server_does_not_have_is_dropped() {
    let mut out = Vec::new();
    for line in [
        "--2026-07-31 10:00:00--  https://x.com/gone.pdf",
        "HTTP request sent, awaiting response... 404 Not Found",
        "2026-07-31 10:00:00 ERROR 404: Not Found.",
        "--2026-07-31 10:00:01--  https://x.com/dead.pdf",
        "Remote file does not exist -- broken link!!!",
        "--2026-07-31 10:00:02--  https://x.com/here.pdf",
        "Length: 10 (10B) [application/pdf]",
        // Spider housekeeping, not a missing resource.
        "Removing x.com/here.pdf.tmp.",
        "unlink: No such file or directory",
    ] {
        collect(line, &mut out);
    }
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].url, "https://x.com/here.pdf");
}

#[test]
fn filters_narrow_the_list() {
    let base = Crawl {
        url: "https://x.com/docs/".into(),
        exts: vec!["pdf".into()],
        min_size: Some(1000.0),
        max_size: Some(10_000.0),
        ..Default::default()
    };

    assert!(base.keep(&found("https://x.com/docs/a.pdf", Some(5000.0))));
    assert!(!base.keep(&found("https://x.com/docs/a.zip", Some(5000.0))), "extension");
    assert!(!base.keep(&found("https://x.com/docs/a.pdf", Some(10.0))), "too small");
    assert!(!base.keep(&found("https://x.com/docs/a.pdf", Some(1e9))), "too big");
    assert!(base.keep(&found("https://x.com/docs/a.pdf", None)), "unknown size is kept");
    assert!(
        !base.keep(&found("https://other.com/a.pdf", Some(5000.0))),
        "another host, and this crawl stays home"
    );

    let anywhere = Crawl { same_domain: false, ..base.clone() };
    assert!(anywhere.keep(&found("https://other.com/a.pdf", Some(5000.0))));

    let patterns = Crawl {
        exts: Vec::new(),
        min_size: None,
        max_size: None,
        include: vec!["/2026/".into()],
        exclude: vec!["*draft*".into()],
        ..base
    };
    assert!(patterns.keep(&found("https://x.com/2026/report.pdf", None)));
    assert!(!patterns.keep(&found("https://x.com/2025/report.pdf", None)), "not included");
    assert!(!patterns.keep(&found("https://x.com/2026/draft-report.pdf", None)), "excluded");
}

#[test]
fn a_pattern_matches_as_a_substring_or_a_glob() {
    assert!(wild("report", "https://x.com/a/report.pdf"));
    assert!(wild("*.pdf", "https://x.com/a/report.pdf"));
    assert!(!wild("*.pdf", "https://x.com/a/report.zip"));
    assert!(wild("https://x.com/*/report*", "https://x.com/a/report.pdf"));
    assert!(!wild("https://y.com/*", "https://x.com/a/report.pdf"), "anchored at the front");
}

#[test]
fn urls_map_to_local_directories_and_safe_names() {
    assert_eq!(local_dir("https://x.com/docs/a/b.pdf", false), "x.com/docs/a");
    assert_eq!(local_dir("https://x.com/b.pdf", false), "x.com");
    assert_eq!(local_dir("https://x.com/docs/a/b.pdf", true), "", "flat mode keeps no structure");
    // A path that tries to climb out of the download directory cannot.
    assert_eq!(local_dir("https://x.com/../../etc/passwd", false), "x.com/etc");

    assert_eq!(local_name("https://x.com/docs/b.pdf"), "b.pdf");
    assert_eq!(local_name("https://x.com/docs/"), "index.html");
    // A query string becomes part of the name instead of an unopenable file.
    let named = local_name("https://x.com/get.php?id=7&fmt=pdf");
    assert_eq!(named, "get@id=7&fmt=pdf.php");
    assert!(!named.contains('/') && !named.contains('?'));
    assert!(local_name("https://x.com/a/../:*?.txt").len() < 40);
}

#[test]
fn the_mirror_flags_cover_an_offline_copy() {
    let crawl = Crawl {
        url: "https://x.com/docs/".into(),
        depth: 3,
        ..Default::default()
    };
    let args = crawl.mirror_args().join(" ");
    for flag in [
        "--recursive",
        "--level=3",
        "--page-requisites",
        "--convert-links",
        "--timestamping",
        "--backup-converted",
        "--no-if-modified-since",
        "--domains=x.com",
    ] {
        assert!(args.contains(flag), "{flag} missing from {args}");
    }
    assert!(!args.contains("--no-directories"), "structure is kept by default");
    assert!(Crawl { flat: true, ..crawl.clone() }.mirror_args().join(" ").contains("--no-directories"));
    assert!(Crawl { same_domain: false, ..crawl.clone() }.mirror_args().join(" ").contains("--span-hosts"));
    assert!(Crawl { under_path: true, ..crawl }.mirror_args().join(" ").contains("--no-parent"));
}

#[test]
fn the_spider_stays_on_the_host_without_being_trapped_under_the_start_path() {
    // The bug this guards: `--no-parent` on a page whose files live in a
    // sibling directory makes wget reject every link and find nothing.
    let crawl = Crawl {
        url: "https://x.com/courses/algo/video_galleries/lectures/".into(),
        exts: vec!["mp4".into()],
        ..Default::default()
    };
    let args = format!("{:?}", crawl.spider_command());
    assert!(args.contains("--domains=x.com"), "the host is still the boundary");
    assert!(!args.contains("--no-parent"), "siblings of the start page are reachable");

    let under = Crawl { under_path: true, ..crawl.clone() };
    assert!(format!("{:?}", under.spider_command()).contains("--no-parent"), "opt-in still works");

    let anywhere = Crawl { same_domain: false, ..crawl };
    let args = format!("{:?}", anywhere.spider_command());
    assert!(args.contains("--span-hosts") && !args.contains("--domains="));
}
