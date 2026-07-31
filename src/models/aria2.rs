use std::path::Path;
use std::process::Command;

use crate::models::backend::Backend;
use crate::models::download::{Overrides, Progress};
use crate::utils::{args, parse};

pub struct Aria2;

/// Direct files, torrents and magnets go to aria2c; anything else is a page
/// that yt-dlp knows how to dig through.
/// extension heuristic, swap for a per-backend probe if it misfires.
const FILE_EXTS: &[&str] = &[
    ".zip", ".tar", ".gz", ".xz", ".zst", ".7z", ".rar", ".iso", ".img", ".deb", ".rpm", ".pkg",
    ".exe", ".msi", ".dmg", ".appimage", ".bin", ".pdf", ".epub", ".jpg", ".png", ".mp3", ".flac",
];

/// A magnet or `.torrent`, as opposed to the direct files aria2 also fetches.
pub fn is_torrent(url: &str) -> bool {
    let url = url.to_lowercase();
    url.starts_with("magnet:") || url.ends_with(".torrent")
}

impl Backend for Aria2 {
    fn name(&self) -> &'static str {
        "aria2c"
    }

    fn accepts(&self, url: &str) -> bool {
        let url = url.to_lowercase();
        if url.starts_with("magnet:") || url.starts_with("ftp://") || url.ends_with(".torrent") {
            return true;
        }
        let path = url.split(['?', '#']).next().unwrap_or(&url);
        FILE_EXTS.iter().any(|e| path.ends_with(e))
    }

    fn command(&self, url: &str, dir: &Path, over: &Overrides) -> Command {
        let mut c = Command::new("aria2c");
        c.arg("--summary-interval=1")
            .arg("--console-log-level=warn")
            .arg("--continue=true")
            .arg("--dir")
            .arg(if over.dir.is_empty() { dir } else { Path::new(&over.dir) });
        c.args(args::load(self.name()));
        // Last, so this item's settings beat the global flags.
        if !over.name.is_empty() {
            c.arg("--out").arg(&over.name);
        }
        if !over.rate.is_empty() {
            c.arg(format!("--max-download-limit={}", over.rate));
        }
        c.arg(url);
        c
    }

    fn parse(&self, line: &str) -> Option<Progress> {
        parse::aria2(line)
    }

    /// aria2 documents its exit codes; `exit 13` on its own tells nobody that
    /// the download stopped because the file was already there.
    fn reason(&self, code: i32) -> String {
        match code {
            2 => "timed out".into(),
            3 => "resource not found".into(),
            5 => "too slow, aria2 gave up".into(),
            6 => "network problem".into(),
            7 => "stopped while unfinished".into(),
            9 => "not enough disk space".into(),
            11 | 12 => "already downloading this torrent".into(),
            13 => "the file already exists".into(),
            15 | 16 => "could not write the file".into(),
            19 => "could not resolve the host".into(),
            24 => "authorization failed".into(),
            code => format!("exit {code}"),
        }
    }

    fn config_flag(&self) -> &'static str {
        "--conf-path"
    }

    /// aria2's config syntax: long options without the dashes.
    fn credentials(&self, user: &str, pass: &str) -> String {
        format!("http-user={user}\nhttp-passwd={pass}\nftp-user={user}\nftp-passwd={pass}\n")
    }
}
