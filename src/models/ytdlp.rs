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

/// Lists the entries — one `<url>\t<date>\t<title>` per line — without
/// touching the media itself. One request for the whole playlist.
///
/// `approximate_date` is what makes a date filter fast: YouTube's index
/// carries "3 years ago" rather than a date, and this turns that into one.
/// The result is coarse — it can be months out — but it arrives with the
/// listing instead of costing a request per entry.
pub fn list_command(url: &str) -> Command {
    let mut c = Command::new("yt-dlp");
    c.arg("--flat-playlist")
        .arg("--ignore-errors")
        .arg("--extractor-args")
        .arg("youtubetab:approximate_date")
        .arg("--print")
        .arg("%(url)s\t%(upload_date)s\t%(title)s")
        .arg(url);
    c
}

/// The slow, exact listing: every entry opened for its real upload date, and
/// yt-dlp itself doing the filtering. Only for what the fast pass cannot
/// answer — a site whose index carries no dates, or a relative date like
/// `now-6months` that has to be resolved against a real one.
pub fn dated_list_command(url: &str, dates: &DateRange) -> Command {
    let mut c = Command::new("yt-dlp");
    // `url` is the media stream once an entry is opened for real; the page is
    // what belongs in the download list.
    c.arg("--ignore-errors").arg("--print").arg("%(webpage_url)s\t%(upload_date)s\t%(title)s");
    if !dates.after.is_empty() {
        c.arg("--dateafter").arg(&dates.after);
    }
    if !dates.before.is_empty() {
        c.arg("--datebefore").arg(&dates.before);
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
    pub entries: Vec<Entry>,
}

/// An upload-date window, as yt-dlp spells one. Either end may be empty,
/// which leaves that side open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DateRange {
    pub after: String,
    pub before: String,
}

impl DateRange {
    pub fn is_empty(&self) -> bool {
        self.after.is_empty() && self.before.is_empty()
    }
}

/// A date as yt-dlp reads it. `2020-01-01` is what people type, `20200101` is
/// what it wants, and its own shorthand — `today`, `now-6months` — contains
/// `-` too, so only a plain date is stripped. Empty stays empty, which leaves
/// that end of the range open.
pub fn date(text: &str) -> String {
    let text = text.trim();
    match text.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '/') {
        true => text.replace(['-', '/'], ""),
        false => text.to_string(),
    }
}

/// One entry of a playlist as the listing describes it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub url: String,
    /// `YYYYMMDD`, or empty when the listing had none to give.
    pub date: String,
    pub title: String,
}

/// One listed line. The date and title are whatever yt-dlp knew: a site with
/// no dates in its index, or a private entry, gives neither.
pub fn entry(line: &str) -> Option<Entry> {
    if !line.starts_with("http") {
        return None;
    }
    let mut parts = line.split('\t');
    let url = parts.next()?.trim().to_string();
    let date = parts.next().unwrap_or("").trim();
    let title = parts.next().unwrap_or("").trim().to_string();
    Some(Entry {
        url,
        // yt-dlp prints NA for a field it has no value for.
        date: match date == "NA" || !date.chars().all(|c| c.is_ascii_digit()) {
            true => String::new(),
            false => date.to_string(),
        },
        title,
    })
}

/// Is this a plain `YYYYMMDD`, and so comparable without asking yt-dlp? Its
/// relative forms — `today`, `now-6months` — are not.
pub fn is_plain_date(text: &str) -> bool {
    text.len() == 8 && text.chars().all(|c| c.is_ascii_digit())
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
