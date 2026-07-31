pub mod args;
pub mod parse;

use std::path::{Path, PathBuf};

/// `$XDG_CONFIG_HOME/muxget`, else `~/.config/muxget`. Themes and per-backend
/// argument files live here.
pub fn config_dir() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
    }
    .join("muxget")
}

/// Expand a C-style url pattern over an inclusive range: `a%03d.iso` with
/// `1-3` gives `a001.iso`, `a002.iso`, `a003.iso`. No range, no expansion.
pub fn expand(pattern: &str, range: &str) -> Vec<String> {
    let Some((from, to)) = parse_range(range) else {
        return vec![pattern.to_string()];
    };
    // A typo like `1-100000000` must not enqueue a million downloads.
    let to = to.min(from + MAX_EXPANSION - 1);
    (from..=to).map(|n| fill(pattern, n)).collect()
}

/// Ceiling on one expansion.
pub const MAX_EXPANSION: i64 = 500;

/// `from-to`, inclusive, non-negative. Anything else is not a range.
pub fn parse_range(range: &str) -> Option<(i64, i64)> {
    let (from, to) = range.trim().split_once('-')?;
    let from: i64 = from.trim().parse().ok()?;
    let to: i64 = to.trim().parse().ok()?;
    (from >= 0 && to >= from).then_some((from, to))
}

/// Substitute `%d` / `%0Nd` with `n`; `%%` is a literal `%`. Anything else
/// after a `%` is left as typed.
pub fn fill(pattern: &str, n: i64) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        // The digits are the pad width, e.g. `%03d`.
        while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
            spec.push(chars.next().expect("peeked"));
        }
        match chars.peek() {
            Some('d') => {
                chars.next();
                let width: usize = spec.parse().unwrap_or(0);
                out.push_str(&format!("{n:0width$}"));
            }
            Some('%') if spec.is_empty() => {
                chars.next();
                out.push('%');
            }
            _ => {
                out.push('%');
                out.push_str(&spec);
            }
        }
    }
    out
}

/// Owner-only directory holding one credentials file per running download.
fn creds_dir() -> PathBuf {
    config_dir().join("creds")
}

/// Write `contents` where only the owner can read it, and return the path.
/// A file rather than the command line, which `ps` shows to everyone.
pub fn write_creds(id: usize, contents: &str) -> Option<PathBuf> {
    let dir = creds_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(id.to_string());
    // 0600 from creation, so there is no readable window.
    let mut file = open_private(&path)?;
    use std::io::Write;
    file.write_all(contents.as_bytes()).ok()?;
    Some(path)
}

#[cfg(unix)]
fn open_private(path: &Path) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .ok()
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> Option<std::fs::File> {
    std::fs::File::create(path).ok()
}

/// Delete one download's credentials file.
pub fn clear_creds(id: usize) {
    let _ = std::fs::remove_file(creds_dir().join(id.to_string()));
}

/// Delete every credentials file. Only safe at startup and exit, when no
/// process is using one.
pub fn clear_all_creds() {
    let _ = std::fs::remove_dir_all(creds_dir());
}
