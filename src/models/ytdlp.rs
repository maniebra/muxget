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
///
/// A date range costs the flat listing: upload dates are not in a playlist's
/// index, so yt-dlp has to open every entry to know one. Without a range this
/// is a single request for the whole playlist.
pub fn list_command(url: &str, dates: &DateRange) -> Command {
    let mut c = Command::new("yt-dlp");
    c.arg("--ignore-errors");
    match dates.is_empty() {
        true => {
            c.arg("--flat-playlist").arg("--print").arg("%(url)s\t%(title)s");
        }
        false => {
            // `url` is the media stream once an entry is opened for real; the
            // page is what belongs in the download list.
            c.arg("--print").arg("%(webpage_url)s\t%(title)s");
            if !dates.after.is_empty() {
                c.arg("--dateafter").arg(&dates.after);
            }
            if !dates.before.is_empty() {
                c.arg("--datebefore").arg(&dates.before);
            }
        }
    }
    c.arg(url);
    c
}

/// A playlist listed but not yet queued, with everything needed to list it
/// again under a different date range.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Listing {
    pub url: String,
    pub queue: usize,
    pub over: Overrides,
    pub dates: DateRange,
    /// `(url, title)` per entry, in playlist order.
    pub entries: Vec<(String, String)>,
}

/// An upload-date window, as yt-dlp spells one. Either end may be empty,
/// which leaves that side open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DateRange {
    pub after: String,
    pub before: String,
}

impl DateRange {
    /// `<from>..<to>`, either side optional. Dates are passed to yt-dlp as
    /// typed once the separators are gone, so its own shorthand — `today`,
    /// `now-6months`, `20200101` — works as well as `2020-01-01`.
    pub fn parse(text: &str) -> DateRange {
        let (after, before) = text.trim().split_once("..").unwrap_or((text.trim(), ""));
        DateRange { after: clean_date(after), before: clean_date(before) }
    }

    pub fn is_empty(&self) -> bool {
        self.after.is_empty() && self.before.is_empty()
    }

    /// The range as it goes back into the field it was typed in.
    pub fn typed(&self) -> String {
        match self.is_empty() {
            true => String::new(),
            false => format!("{}..{}", self.after, self.before),
        }
    }
}

/// `2020-01-01` is what people type; `20200101` is what yt-dlp reads. Its own
/// relative forms contain `-` too, so only a plain date is stripped.
fn clean_date(text: &str) -> String {
    let text = text.trim();
    match text.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '/') {
        true => text.replace(['-', '/'], ""),
        false => text.to_string(),
    }
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
