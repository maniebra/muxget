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
