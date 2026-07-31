use std::path::Path;
use std::process::Command;

use crate::models::backend::Backend;
use crate::models::download::{Overrides, Progress};
use crate::utils::{args, parse};

pub struct Aria2;

/// Direct files, torrents and magnets go to aria2c; anything else is a page
/// that yt-dlp knows how to dig through.
// ponytail: extension heuristic, swap for a per-backend probe if it misfires.
const FILE_EXTS: &[&str] = &[
    ".zip", ".tar", ".gz", ".xz", ".zst", ".7z", ".rar", ".iso", ".img", ".deb", ".rpm", ".pkg",
    ".exe", ".msi", ".dmg", ".appimage", ".bin", ".pdf", ".epub", ".jpg", ".png", ".mp3", ".flac",
];

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
        // This item's own settings come last, beating the global flags.
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
}
