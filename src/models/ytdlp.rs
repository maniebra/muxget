use std::path::Path;
use std::process::Command;

use crate::models::backend::Backend;
use crate::models::download::Progress;
use crate::utils::parse;

pub struct YtDlp;

impl Backend for YtDlp {
    fn name(&self) -> &'static str {
        "yt-dlp"
    }

    /// Fallback for every remaining http(s) url — aria2c is checked first.
    fn accepts(&self, url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    fn command(&self, url: &str, dir: &Path) -> Command {
        let mut c = Command::new("yt-dlp");
        c.arg("--newline")
            .arg("--no-color")
            .arg("--continue")
            .arg("-P")
            .arg(dir)
            .arg(url);
        c
    }

    fn parse(&self, line: &str) -> Option<Progress> {
        parse::ytdlp(line)
    }
}
