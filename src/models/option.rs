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
}

use Kind::{Flag, Value};

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
    OptSpec { flag: "-f", label: "format selector", kind: Value, hint: "e.g. bv*+ba/b" },
    OptSpec { flag: "-o", label: "output template", kind: Value, hint: "%(title)s.%(ext)s" },
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

pub fn specs(backend: &str) -> &'static [OptSpec] {
    match backend {
        "aria2c" => ARIA2,
        _ => YTDLP,
    }
}
