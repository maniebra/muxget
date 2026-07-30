use std::path::PathBuf;

use ratatui::style::Color;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub accent: Color,
    pub ok: Color,
    pub err: Color,
    pub muted: Color,
    /// Selected row background.
    pub selected: Color,
    /// Window background.
    pub bg: Color,
    /// Sidebar / alternating row background.
    pub panel: Color,
    pub fg: Color,
}

/// name, accent, ok, err, muted, selected, bg, panel, fg
const BUILTIN: &[[&str; 9]] = &[
    ["tokyonight", "#7aa2f7", "#9ece6a", "#f7768e", "#565f89", "#292e42", "#1a1b26", "#16161e", "#c0caf5"],
    ["catppuccin", "#89b4fa", "#a6e3a1", "#f38ba8", "#6c7086", "#313244", "#1e1e2e", "#181825", "#cdd6f4"],
    ["monokai", "#66d9ef", "#a6e22e", "#f92672", "#75715e", "#3e3d32", "#272822", "#1e1f1c", "#f8f8f2"],
    ["gruvbox", "#83a598", "#b8bb26", "#fb4934", "#928374", "#3c3836", "#282828", "#1d2021", "#ebdbb2"],
    ["nord", "#88c0d0", "#a3be8c", "#bf616a", "#4c566a", "#3b4252", "#2e3440", "#272c36", "#d8dee9"],
    ["dracula", "#8be9fd", "#50fa7b", "#ff5555", "#6272a4", "#44475a", "#282a36", "#21222c", "#f8f8f2"],
];

impl Default for Theme {
    fn default() -> Self {
        Theme::from_row(&BUILTIN[0])
    }
}

impl Theme {
    fn from_row(row: &[&str; 9]) -> Theme {
        let c = |i: usize| hex(row[i]).unwrap_or(Color::Reset);
        Theme {
            name: row[0].to_string(),
            accent: c(1),
            ok: c(2),
            err: c(3),
            muted: c(4),
            selected: c(5),
            bg: c(6),
            panel: c(7),
            fg: c(8),
        }
    }

    /// `key = "#rrggbb"` lines; `#` comments, unknown keys and bad colors are
    /// ignored so a typo degrades to the default color instead of a crash.
    pub fn from_config(name: &str, body: &str) -> Theme {
        let mut t = Theme {
            name: name.to_string(),
            ..Theme::default()
        };
        for line in body.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some(color) = hex(value.trim().trim_matches(['"', '\''])) else {
                continue;
            };
            match key.trim() {
                "accent" => t.accent = color,
                "ok" => t.ok = color,
                "err" => t.err = color,
                "muted" => t.muted = color,
                "selected" => t.selected = color,
                "bg" => t.bg = color,
                "panel" => t.panel = color,
                "fg" => t.fg = color,
                _ => {}
            }
        }
        t
    }

    /// Built-ins plus `<config>/themes/*.toml`; a user file wins on name clash.
    pub fn all() -> Vec<Theme> {
        let mut themes: Vec<Theme> = BUILTIN.iter().map(Theme::from_row).collect();
        let Ok(entries) = std::fs::read_dir(config_dir().join("themes")) else {
            return themes;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let (Some(name), Ok(body)) = (path.file_stem(), std::fs::read_to_string(&path)) else {
                continue;
            };
            let theme = Theme::from_config(&name.to_string_lossy(), &body);
            match themes.iter().position(|t| t.name == theme.name) {
                Some(i) => themes[i] = theme,
                None => themes.push(theme),
            }
        }
        themes
    }

    /// Unknown name falls back to the default rather than failing a launch.
    pub fn named(name: &str) -> Theme {
        Theme::all()
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap_or_default()
    }

    pub fn next(&self, themes: &[Theme]) -> Theme {
        if themes.is_empty() {
            return self.clone();
        }
        let i = themes.iter().position(|t| t.name == self.name).unwrap_or(0);
        themes[(i + 1) % themes.len()].clone()
    }

    /// Remembered across runs; failure is not worth interrupting a download for.
    pub fn save(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("theme"), &self.name);
    }

    pub fn saved() -> Option<Theme> {
        let name = std::fs::read_to_string(config_dir().join("theme")).ok()?;
        Some(Theme::named(name.trim()))
    }
}

fn config_dir() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
    }
    .join("muxget")
}

fn hex(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}
