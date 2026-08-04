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

/// Is this program on `PATH`? No spawning: a missing backend would cost a
/// failed process launch to find out, and a present one a whole `--version`.
pub fn on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| {
        // `.exe` for Windows, where the bare name is not the file name.
        [program, &format!("{program}.exe")]
            .iter()
            .any(|name| dir.join(name).is_file())
    })
}

/// Make sure a download can actually be written into `dir`: create it if it
/// is missing, then prove a file can be made in it. Returns a message fit for
/// the status line, since "permission denied" three levels down in a
/// backend's own error is not one.
///
/// The probe is the point: a directory that already exists but is not
/// writable creates fine and fails later, once the download has started and
/// looks like it is working.
pub fn prepare_dir(dir: &Path) -> Result<(), String> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Err(format!("cannot create {}: {}", dir.display(), reason(&e)));
    }
    let probe = dir.join(".muxget-write-test");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(format!("cannot write into {}: {}", dir.display(), reason(&e))),
    }
}

/// An io error in the words a person uses.
fn reason(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "permission denied".into(),
        std::io::ErrorKind::NotFound => "no such directory".into(),
        std::io::ErrorKind::StorageFull => "the disk is full".into(),
        // `Read-only file system` and friends read well enough as they are.
        _ => e.to_string(),
    }
}

/// The system clipboard, through whichever tool this desktop has. No X11 or
/// Wayland library for one read: the tools are what a user already has, and
/// the first one on `PATH` wins.
pub fn clipboard() -> Option<String> {
    const READERS: [(&str, &[&str]); 5] = [
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-o", "-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--output"]),
        ("pbpaste", &[]),
        ("powershell.exe", &["-NoProfile", "-Command", "Get-Clipboard"]),
    ];
    let (program, args) = READERS.into_iter().find(|(p, _)| on_path(p))?;
    let out = std::process::Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Urls in pasted text, in the order they appear and without repeats. One per
/// line: a line that is not a url is a note, a title or a stray word, and
/// queueing it would only fail.
pub fn urls_in(text: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let is_url = ["http://", "https://", "ftp://", "ftps://", "magnet:?"]
            .iter()
            .any(|scheme| line.starts_with(scheme));
        if is_url && !urls.iter().any(|u| u == line) {
            urls.push(line.to_string());
        }
    }
    urls
}

/// Backends muxget drives that are not installed, in registry order. Empty is
/// the happy case, so a caller can treat it as a boolean.
pub fn missing_backends() -> Vec<&'static str> {
    crate::models::backends()
        .iter()
        .map(|b| b.name())
        .filter(|name| !on_path(name))
        .collect()
}

/// Kill a backend process left over from an earlier run. The name check keeps
/// a recycled pid from taking some unrelated process down with it; `ps` rather
/// than `/proc` so this also holds on macOS.
pub fn reap(pid: u32, name: &str) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "args="])
        .output();
    let Ok(out) = out else { return false };
    if !String::from_utf8_lossy(&out.stdout).contains(name) {
        return false;
    }
    crate::models::download::signal(pid, "-KILL")
}

/// Today as `YYYYMMDD`, UTC — the form yt-dlp reads dates in. Days since the
/// epoch turned into a civil date by Howard Hinnant's algorithm, which is
/// exact for every date this will ever see and shorter than a date crate.
pub fn today() -> String {
    let (y, m, d) = civil(today_days());
    format!("{y:04}{m:02}{d:02}")
}

/// Today as days since the epoch, which is what date arithmetic wants.
pub fn today_days() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    secs as i64 / 86_400
}

/// A `YYYYMMDD` as days since the epoch, so two dates can be subtracted.
/// `None` for anything that is not one.
pub fn days_of(date: &str) -> Option<i64> {
    if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = |range: std::ops::Range<usize>| date[range].parse::<i64>().ok();
    Some(days_from_civil(n(0..4)?, n(4..6)?, n(6..8)?))
}

/// The inverse of [`civil`], by the same era arithmetic.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days since 1970-01-01 as (year, month, day). Eras of 400 years, each of
/// which has exactly 146097 days, so the leap rules need no special cases.
pub fn civil(days: i64) -> (i64, i64, i64) {
    // Shift the epoch to 0000-03-01, which puts the leap day last in a year.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Only `~`, which is what a typed path actually needs; the shell is not
/// involved here so nothing else would be expanded anyway.
pub fn expand_home(path: &str) -> String {
    match path.strip_prefix('~') {
        Some(rest) => {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}{rest}")
        }
        None => path.to_string(),
    }
}
