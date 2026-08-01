/// A backend option the TUI knows how to present. Anything not listed here
/// still survives round-trips — see `controllers::options`.
pub struct OptSpec {
    pub flag: &'static str,
    pub label: &'static str,
    /// `Flag` is on/off; `Value` carries text after `=`.
    pub kind: Kind,
    pub hint: &'static str,
}

#[derive(PartialEq)]
pub enum Kind {
    Flag,
    Value,
    /// A short list of named settings for a flag whose real value nobody
    /// wants to type. Space cycles through them; `x` clears the flag.
    Choice(&'static [Preset]),
}

/// One entry of a `Choice`: what the user reads, and what the backend gets.
#[derive(PartialEq)]
pub struct Preset {
    pub label: &'static str,
    pub value: &'static str,
}

use Kind::{Choice, Flag, Value};

/// Video quality as a yt-dlp format selector. Each one asks for the best
/// video at or below a height plus the best audio, falling back to whatever
/// single file comes closest — the `/b` half matters on sites that do not
/// offer separate streams.
pub const QUALITY: &[Preset] = &[
    Preset { label: "best available", value: "bv*+ba/b" },
    Preset { label: "1080p", value: "bv*[height<=1080]+ba/b[height<=1080]" },
    Preset { label: "720p", value: "bv*[height<=720]+ba/b[height<=720]" },
    Preset { label: "480p", value: "bv*[height<=480]+ba/b[height<=480]" },
    Preset { label: "360p", value: "bv*[height<=360]+ba/b[height<=360]" },
    Preset { label: "smallest file", value: "wv*+wa/w" },
    Preset { label: "audio only", value: "ba/b" },
];

pub const ARIA2: &[OptSpec] = &[
    OptSpec { flag: "--split", label: "connections per download", kind: Value, hint: "1-16" },
    OptSpec { flag: "--max-connection-per-server", label: "connections per server", kind: Value, hint: "1-16" },
    OptSpec { flag: "--min-split-size", label: "minimum split size", kind: Value, hint: "e.g. 1M" },
    OptSpec { flag: "--max-download-limit", label: "speed limit", kind: Value, hint: "e.g. 2M, 0 = off" },
    OptSpec { flag: "--max-tries", label: "retries", kind: Value, hint: "0 = forever" },
    OptSpec { flag: "--retry-wait", label: "retry wait (s)", kind: Value, hint: "seconds" },
    OptSpec { flag: "--timeout", label: "timeout (s)", kind: Value, hint: "seconds" },
    OptSpec { flag: "--user-agent", label: "user agent", kind: Value, hint: "sent as-is" },
    OptSpec { flag: "--referer", label: "referer", kind: Value, hint: "url" },
    OptSpec { flag: "--header", label: "extra header", kind: Value, hint: "Name: value" },
    OptSpec { flag: "--all-proxy", label: "proxy", kind: Value, hint: "http://host:port" },
    OptSpec { flag: "--file-allocation", label: "file allocation", kind: Value, hint: "none|prealloc|falloc" },
    OptSpec { flag: "--check-integrity", label: "verify checksums", kind: Flag, hint: "" },
    OptSpec { flag: "--auto-file-renaming=false", label: "overwrite instead of renaming", kind: Flag, hint: "" },
    OptSpec { flag: "--remote-time", label: "keep remote timestamp", kind: Flag, hint: "" },
    OptSpec { flag: "--seed-time=0", label: "torrents: stop seeding at 100%", kind: Flag, hint: "" },
];

pub const YTDLP: &[OptSpec] = &[
    // Long forms: yt-dlp reads `--format=x`, but `-f=x` would hand it the
    // literal `=x` and silently download the default quality instead.
    OptSpec { flag: "--format", label: "video quality", kind: Choice(QUALITY), hint: "space cycles" },
    OptSpec { flag: "--output", label: "output template", kind: Value, hint: "%(title)s.%(ext)s" },
    OptSpec { flag: "--limit-rate", label: "speed limit", kind: Value, hint: "e.g. 2M" },
    OptSpec { flag: "--retries", label: "retries", kind: Value, hint: "number or infinite" },
    OptSpec { flag: "--cookies-from-browser", label: "cookies from browser", kind: Value, hint: "firefox|chrome|…" },
    OptSpec { flag: "--proxy", label: "proxy", kind: Value, hint: "http://host:port" },
    OptSpec { flag: "--sub-langs", label: "subtitle languages", kind: Value, hint: "en,fa or all" },
    OptSpec { flag: "--extract-audio", label: "audio only", kind: Flag, hint: "" },
    OptSpec { flag: "--write-subs", label: "download subtitles", kind: Flag, hint: "" },
    OptSpec { flag: "--embed-thumbnail", label: "embed thumbnail", kind: Flag, hint: "" },
    OptSpec { flag: "--embed-metadata", label: "embed metadata", kind: Flag, hint: "" },
    OptSpec { flag: "--no-playlist", label: "single video, not playlist", kind: Flag, hint: "" },
];

/// Crawl-wide wget settings; a crawl's own recursion flags come from its form.
pub const WGET: &[OptSpec] = &[
    OptSpec { flag: "--limit-rate", label: "speed limit", kind: Value, hint: "e.g. 2M" },
    OptSpec { flag: "--wait", label: "wait between requests (s)", kind: Value, hint: "seconds" },
    OptSpec { flag: "--user-agent", label: "user agent", kind: Value, hint: "sent as-is" },
    OptSpec { flag: "--header", label: "extra header", kind: Value, hint: "Name: value" },
    OptSpec { flag: "--reject", label: "never fetch", kind: Value, hint: "e.g. *.exe,*.zip" },
    OptSpec { flag: "--exclude-directories", label: "skip directories", kind: Value, hint: "/cgi-bin,/tmp" },
    OptSpec { flag: "--continue", label: "resume partial files", kind: Flag, hint: "" },
    OptSpec { flag: "-e robots=off", label: "ignore robots.txt", kind: Flag, hint: "" },
    OptSpec { flag: "--no-check-certificate", label: "skip certificate checks", kind: Flag, hint: "" },
];

/// Defaults for the crawl form, stored the same way a backend's flags are.
/// These are not passed to wget: a crawl's own words override them, and
/// `controllers::crawl` turns the result into flags.
pub const CRAWL: &[OptSpec] = &[
    OptSpec { flag: "depth", label: "depth", kind: Value, hint: "how many links deep" },
    OptSpec { flag: "extensions", label: "extensions", kind: Value, hint: "e.g. pdf,mp4" },
    OptSpec { flag: "size", label: "size min-max", kind: Value, hint: "e.g. 1M-500M" },
    OptSpec { flag: "any-domain", label: "follow links off the host", kind: Flag, hint: "" },
    OptSpec { flag: "under-path", label: "stay under the start url", kind: Flag, hint: "" },
    OptSpec { flag: "no-robots", label: "ignore robots.txt and nofollow", kind: Flag, hint: "" },
    OptSpec { flag: "flat", label: "save without the directory tree", kind: Flag, hint: "" },
];

pub fn specs(backend: &str) -> &'static [OptSpec] {
    match backend {
        "aria2c" => ARIA2,
        "wget" => WGET,
        "crawl" => CRAWL,
        _ => YTDLP,
    }
}
