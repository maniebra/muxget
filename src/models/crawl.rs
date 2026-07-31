use std::process::Command;

use crate::utils::parse::bytes;

/// A crawl as the form describes it. wget does the crawling; the filters
/// below decide which of what it finds is worth downloading.
#[derive(Debug, Clone, PartialEq)]
pub struct Crawl {
    pub url: String,
    /// How many links deep to follow. `0` is the page itself.
    pub depth: u8,
    /// Extensions to keep, without dots. Empty keeps every type.
    pub exts: Vec<String>,
    /// Url patterns to keep and to drop; `*` is a wildcard, anything else is
    /// matched as a substring. Excludes win over includes.
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub min_size: Option<f64>,
    pub max_size: Option<f64>,
    /// Stay on the page's own host and below its path.
    pub same_domain: bool,
    /// Drop the url's directories and save everything side by side.
    pub flat: bool,
    /// Mirror the site for offline reading — pages plus the stylesheets,
    /// scripts, images and fonts they need, with links rewritten to point at
    /// the local copies — instead of listing files to pick from.
    pub offline: bool,
}

/// One url the crawl turned up, with the size the server reported for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub url: String,
    pub size: Option<f64>,
}

impl Default for Crawl {
    fn default() -> Crawl {
        Crawl {
            url: String::new(),
            depth: 1,
            exts: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            min_size: None,
            max_size: None,
            same_domain: true,
            flat: false,
            offline: false,
        }
    }
}

impl Crawl {
    /// wget flags shared by the discovery pass and the offline mirror: how
    /// deep to go, and how far off the original page it may wander.
    fn walk_args(&self) -> Vec<String> {
        let mut args = vec![
            "--recursive".to_string(),
            format!("--level={}", self.depth),
            // Politeness, and a crawl that gives up rather than hanging.
            "--tries=2".to_string(),
            "--timeout=20".to_string(),
            "--wait=0.2".to_string(),
        ];
        match self.same_domain {
            true => {
                args.push("--no-parent".into());
                if let Some(host) = host(&self.url) {
                    args.push(format!("--domains={host}"));
                }
            }
            false => args.push("--span-hosts".into()),
        }
        if !self.exts.is_empty() {
            args.push(format!("--accept={}", self.exts.join(",")));
        }
        args
    }

    /// Walk the site without saving anything, so the links can be shown
    /// before a single byte is downloaded.
    pub fn spider_command(&self) -> Command {
        let mut c = Command::new("wget");
        c.arg("--spider").args(self.walk_args()).arg(&self.url);
        c
    }

    /// Flags for the offline mirror: page requisites, local link rewriting,
    /// timestamps so a re-run only fetches what changed, and file names that
    /// survive a query string.
    pub fn mirror_args(&self) -> Vec<String> {
        let mut args = self.walk_args();
        args.extend(
            [
                "--page-requisites",
                "--convert-links",
                // Keeps the untouched original next to the rewritten copy;
                // without it every re-crawl sees its own edits as a change.
                "--backup-converted",
                "--adjust-extension",
                "--timestamping",
                // A conditional request returns no body, and wget needs the
                // page's links to carry on walking. Without this a re-crawl
                // of an unchanged front page stops at the front page.
                "--no-if-modified-since",
                "--restrict-file-names=windows",
            ]
            .map(String::from),
        );
        if self.flat {
            args.push("--no-directories".into());
        }
        args
    }

    /// Does this url survive the filters? Size is only judged when the server
    /// reported one — an unknown size is not a reason to drop a file.
    pub fn keep(&self, f: &Found) -> bool {
        let url = f.url.to_lowercase();
        let path = url.split(['?', '#']).next().unwrap_or(&url);
        if !self.exts.is_empty() && !self.exts.iter().any(|e| path.ends_with(&format!(".{e}"))) {
            return false;
        }
        if self.exclude.iter().any(|p| wild(p, &url)) {
            return false;
        }
        if !self.include.is_empty() && !self.include.iter().any(|p| wild(p, &url)) {
            return false;
        }
        if self.same_domain && host(&self.url) != host(&f.url) {
            return false;
        }
        match f.size {
            None => true,
            Some(size) => {
                self.min_size.is_none_or(|min| size >= min)
                    && self.max_size.is_none_or(|max| size <= max)
            }
        }
    }
}

/// Read wget's log as it walks. A `--date--  url` line opens an entry and the
/// `Length:` line that follows sizes it; every url is reported once, so the
/// same file found twice is a single entry.
pub fn collect(line: &str, found: &mut Vec<Found>) {
    if let Some((_, url)) = line.split_once("--  ") {
        let url = url.trim();
        if url.starts_with("http") && !found.iter().any(|f| f.url == url) {
            found.push(Found { url: url.to_string(), size: None });
        }
        return;
    }
    if let Some(rest) = line.trim().strip_prefix("Length: ") {
        if let Some(last) = found.last_mut() {
            last.size = rest.split_whitespace().next().and_then(bytes);
        }
        return;
    }
    // The url just above this one is not there; offering it would queue a
    // download that can only fail. `broken link` is the spider's own verdict.
    if line.contains(" ERROR ") || line.contains("broken link") {
        found.pop();
    }
}

/// Urls the crawler fetches to do its job rather than because they were
/// linked. Nobody asked for the robots file.
pub fn is_plumbing(url: &str) -> bool {
    url.ends_with("/robots.txt")
}

/// Where a discovered url is saved under the download directory, so the local
/// copy mirrors the site: `https://x.com/docs/a/b.pdf` → `x.com/docs/a`.
/// Flat mode returns nothing and everything lands side by side.
pub fn local_dir(url: &str, flat: bool) -> String {
    if flat {
        return String::new();
    }
    let Some(host) = host(url) else { return String::new() };
    let path = after_host(url);
    let dirs = path.rsplit_once('/').map_or("", |(dirs, _)| dirs);
    let mut out = safe(&host);
    for part in dirs.split('/').filter(|p| !p.is_empty() && *p != "..") {
        out.push('/');
        out.push_str(&safe(part));
    }
    out
}

/// The file name for a url. A query string is folded into the name rather
/// than left to make an unopenable file, and a url ending in `/` becomes
/// `index.html`, as wget's own mirrors do.
pub fn local_name(url: &str) -> String {
    let path = after_host(url);
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let name = path.rsplit('/').next().unwrap_or("");
    let mut name = match name.is_empty() {
        true => "index.html".to_string(),
        false => safe(name),
    };
    if !query.is_empty() {
        // Before the extension, so the file still opens with the right thing.
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name.as_str(), ""));
        name = match ext.is_empty() {
            true => format!("{stem}@{}", safe(query)),
            false => format!("{stem}@{}.{ext}", safe(query)),
        };
    }
    name
}

/// Everything a file name must not contain, on any of the three platforms,
/// plus a length cap so a long query string cannot overrun the file system.
fn safe(part: &str) -> String {
    let cleaned: String = part
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_matches([' ', '.']).to_string();
    match cleaned.is_empty() {
        true => "_".into(),
        false => cleaned.chars().take(120).collect(),
    }
}

/// The host of a url, lowercased and without credentials or port.
pub fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    let host = authority.split(':').next()?;
    (!host.is_empty()).then(|| host.to_lowercase())
}

fn after_host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    match rest.find('/') {
        Some(at) => rest[at + 1..].split('#').next().unwrap_or(""),
        None => "",
    }
}

/// `*` matches any run of characters; a pattern without one is a plain
/// substring, which is what a typed filter usually means.
pub fn wild(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    if !pattern.contains('*') {
        return text.contains(&pattern);
    }
    let mut at = 0;
    let parts: Vec<&str> = pattern.split('*').collect();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(found) = text[at..].find(part) else { return false };
        // A pattern not starting with `*` is anchored at the front, and one
        // not ending with `*` at the back.
        if i == 0 && found != 0 {
            return false;
        }
        at += found + part.len();
    }
    parts.last().is_none_or(|last| last.is_empty() || text.ends_with(last))
}
