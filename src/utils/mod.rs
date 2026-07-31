pub mod args;
pub mod parse;

use std::path::PathBuf;

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
/// `1-3` gives `a001.iso`, `a002.iso`, `a003.iso`. `%%` is a literal `%`.
/// An empty range, or a pattern with no `%d`, yields the pattern unchanged.
pub fn expand(pattern: &str, range: &str) -> Vec<String> {
    let Some((from, to)) = parse_range(range) else {
        return vec![pattern.to_string()];
    };
    // A typo like `1-100000000` should not enqueue a million downloads.
    let to = to.min(from + MAX_EXPANSION - 1);
    (from..=to).map(|n| fill(pattern, n)).collect()
}

/// Ceiling on one expansion, so a mistyped range cannot flood the queue.
pub const MAX_EXPANSION: i64 = 500;

/// `from-to`, inclusive, non-negative. Anything else is not a range.
pub fn parse_range(range: &str) -> Option<(i64, i64)> {
    let (from, to) = range.trim().split_once('-')?;
    let from: i64 = from.trim().parse().ok()?;
    let to: i64 = to.trim().parse().ok()?;
    (from >= 0 && to >= from).then_some((from, to))
}

/// Substitute `%d` / `%0Nd` with `n`; `%%` is a literal `%`, and anything else
/// after a `%` is left alone rather than guessed at.
pub fn fill(pattern: &str, n: i64) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        // Zeros here are the pad width, e.g. `%03d`.
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
