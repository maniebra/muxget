use std::path::Path;
use std::process::Command;

use crate::models::backend::Backend;
use crate::models::download::{Overrides, Progress};
use crate::utils::{args, parse};

pub struct YtDlp;

/// A url yt-dlp would treat as a playlist, channel or mix.
pub fn is_playlist(url: &str) -> bool {
    let url = url.to_lowercase();
    url.contains("list=")
        || url.contains("/playlist")
        || url.contains("/channel/")
        || url.contains("/@")
        || url.ends_with("/videos")
}

/// Expand a playlist into its entries rather than downloading it as one job —
/// unless the user asked yt-dlp for single videos, in which case respect that.
pub fn expands_playlist(url: &str) -> bool {
    is_playlist(url) && !args::load("yt-dlp").iter().any(|a| a == "--no-playlist")
}

/// Lists the entries, one `<url>\t<title>` per line, without touching the
/// media itself.
pub fn list_command(url: &str) -> Command {
    let mut c = Command::new("yt-dlp");
    c.arg("--flat-playlist")
        .arg("--ignore-errors")
        .arg("--print")
        .arg("%(url)s\t%(title)s")
        .arg(url);
    c
}

/// One listed line as (url, title); the title is whatever yt-dlp knew, and
/// may be missing on an old version or a private entry.
pub fn entry(line: &str) -> Option<(String, String)> {
    if !line.starts_with("http") {
        return None;
    }
    let (url, title) = line.split_once('\t').unwrap_or((line, ""));
    Some((url.trim().to_string(), title.trim().to_string()))
}

/// Only what would end the quoted string or escape the next character.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

impl Backend for YtDlp {
    fn name(&self) -> &'static str {
        "yt-dlp"
    }

    /// Fallback for every remaining http(s) url — aria2c is checked first.
    fn accepts(&self, url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    fn command(&self, url: &str, dir: &Path, over: &Overrides) -> Command {
        let mut c = Command::new("yt-dlp");
        c.arg("--newline")
            .arg("--no-color")
            .arg("--continue")
            .arg("-P")
            .arg(if over.dir.is_empty() { dir } else { Path::new(&over.dir) });
        c.args(args::load(self.name()));
        // Last, so this item's settings beat the global flags.
        if !over.name.is_empty() {
            c.arg("-o").arg(&over.name);
        }
        if !over.rate.is_empty() {
            c.arg("-r").arg(&over.rate);
        }
        c.arg(url);
        c
    }

    fn parse(&self, line: &str) -> Option<Progress> {
        parse::ytdlp(line)
    }

    fn config_flag(&self) -> &'static str {
        "--config-location"
    }

    /// A yt-dlp config file holds command-line flags; quotes keep a password
    /// with spaces or `#` intact.
    fn credentials(&self, user: &str, pass: &str) -> String {
        format!("-u \"{}\"\n-p \"{}\"\n", escape(user), escape(pass))
    }
}
