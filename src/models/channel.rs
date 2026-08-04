use crate::utils::config_dir;

/// A channel to keep up with: where it is, and the day it was last synced.
/// Syncing lists everything uploaded since that day and moves it to today.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Channel {
    pub url: String,
    /// `YYYYMMDD`, or empty — which means "everything", the first sync.
    pub last_sync: String,
}

/// `$XDG_CONFIG_HOME/muxget/channels`, in the same subset of TOML the rules
/// file uses: `[[channel]]` headers and `key = "value"`, `#` comments.
pub fn load() -> Vec<Channel> {
    parse(&std::fs::read_to_string(path()).unwrap_or_default())
}

pub fn parse(text: &str) -> Vec<Channel> {
    let mut channels: Vec<Channel> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line == "[[channel]]" {
            channels.push(Channel::default());
            continue;
        }
        let (Some(channel), Some((key, value))) = (channels.last_mut(), line.split_once('=')) else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']).to_string();
        match key.trim() {
            "url" => channel.url = value,
            // A hand-written `2024-01-01` is as good as `20240101`.
            "last_sync" => channel.last_sync = crate::models::ytdlp::date(&value),
            _ => {}
        }
    }
    // A channel with no url has nothing to sync.
    channels.retain(|c| !c.url.is_empty());
    channels
}

pub fn render(channels: &[Channel]) -> String {
    let mut text = String::from("# muxget channel sync\n");
    for c in channels.iter().filter(|c| !c.url.is_empty()) {
        text.push_str(&format!(
            "\n[[channel]]\nurl = \"{}\"\nlast_sync = \"{}\"\n",
            c.url, c.last_sync
        ));
    }
    text
}

pub fn save(channels: &[Channel]) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(path(), render(channels))
}

pub fn path() -> std::path::PathBuf {
    config_dir().join("channels")
}
