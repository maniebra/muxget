use std::path::PathBuf;

use crate::utils::config_dir;

/// Extra flags handed to a backend verbatim, one file per backend
/// (`<config>/muxget/aria2c.args`). Every option the tool supports is reachable
/// this way — muxget never needs to know what they mean.
pub fn path(backend: &str) -> PathBuf {
    config_dir().join(format!("{backend}.args"))
}

/// Whitespace-separated tokens; `#` starts a comment line.
pub fn parse(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .flat_map(|l| l.split_whitespace())
        .map(str::to_string)
        .collect()
}

/// Tokens as (flag, value) pairs; a bare flag gets an empty value. Order is
/// preserved so options the TUI does not know about survive a round-trip.
pub fn to_pairs(tokens: &[String]) -> Vec<(String, String)> {
    tokens
        .iter()
        .map(|t| match t.split_once('=') {
            Some((flag, value)) => (flag.to_string(), value.to_string()),
            None => (t.clone(), String::new()),
        })
        .collect()
}

pub fn render(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(flag, value)| {
            if value.is_empty() {
                flag.clone()
            } else {
                format!("{flag}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn load(backend: &str) -> Vec<String> {
    parse(&raw(backend))
}

/// File contents as typed, for the editor dialog.
pub fn raw(backend: &str) -> String {
    std::fs::read_to_string(path(backend)).unwrap_or_default()
}

pub fn save(backend: &str, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(path(backend), text.trim())
}
