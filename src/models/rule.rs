use crate::utils::{config_dir, parse::bytes};

/// One routing rule. Every condition that is set has to match, so a rule with
/// both `extensions` and `domains` means "this kind of file, from there".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rule {
    pub extensions: Vec<String>,
    pub domains: Vec<String>,
    /// Bytes. Only checkable once the backend reports a total, so a rule that
    /// sets it routes the download after it starts rather than on the way in.
    pub min_size: Option<f64>,
    pub queue: Option<String>,
    pub directory: Option<String>,
    pub backend: Option<String>,
}

/// The rule's fields, in the order the panel shows them.
pub const FIELDS: [&str; 6] =
    ["extensions", "domains", "min_size", "queue", "directory", "backend"];

impl Rule {
    /// One field as text, for the panel and for the file.
    pub fn get(&self, field: usize) -> String {
        match field {
            0 => self.extensions.join(", "),
            1 => self.domains.join(", "),
            2 => self.min_size.map(crate::utils::parse::human).unwrap_or_default(),
            3 => self.queue.clone().unwrap_or_default(),
            4 => self.directory.clone().unwrap_or_default(),
            _ => self.backend.clone().unwrap_or_default(),
        }
    }

    /// One field from text. Empty clears it, which is how a condition or a
    /// destination is taken back off a rule.
    pub fn set(&mut self, field: usize, value: &str) {
        let value = value.trim();
        let listed = || {
            value
                .split(',')
                .map(|v| v.trim().trim_start_matches('.').to_lowercase())
                .filter(|v| !v.is_empty())
                .collect()
        };
        let text = || (!value.is_empty()).then(|| value.to_string());
        match field {
            0 => self.extensions = listed(),
            1 => self.domains = listed(),
            2 => self.min_size = bytes(value),
            3 => self.queue = text(),
            4 => self.directory = text(),
            _ => self.backend = text(),
        }
    }

    /// What the rule matches and where it sends it, for one line of display.
    pub fn summary(&self) -> (String, String) {
        let mut what = Vec::new();
        if !self.extensions.is_empty() {
            what.push(self.extensions.join(", "));
        }
        if !self.domains.is_empty() {
            what.push(self.domains.join(", "));
        }
        if let Some(min) = self.min_size {
            what.push(format!("over {}", crate::utils::parse::human(min)));
        }
        let mut to = Vec::new();
        if let Some(q) = &self.queue {
            to.push(format!("queue {q}"));
        }
        if let Some(d) = &self.directory {
            to.push(d.clone());
        }
        if let Some(b) = &self.backend {
            to.push(b.clone());
        }
        (what.join(" + "), to.join(" · "))
    }
}

impl Rule {
    /// Does the url alone satisfy this rule? A `min_size` is not answered
    /// here — see [`Rule::wants_size`].
    pub fn matches(&self, url: &str) -> bool {
        if self.extensions.is_empty() && self.domains.is_empty() {
            // A size-only rule matches every url and waits for the total.
            return self.min_size.is_some();
        }
        let url = url.to_lowercase();
        let path = url.split(['?', '#']).next().unwrap_or(&url);
        let ext_ok = self.extensions.is_empty()
            || self.extensions.iter().any(|e| path.ends_with(&format!(".{e}")));
        let domain_ok = self.domains.is_empty() || self.domains.iter().any(|d| url.contains(d));
        ext_ok && domain_ok
    }

    pub fn wants_size(&self, total: f64) -> bool {
        self.min_size.is_none_or(|min| total >= min)
    }
}

/// `$XDG_CONFIG_HOME/muxget/rules`, in the subset of TOML the format needs:
/// `[[rule]]` headers, `key = "value"` and `key = ["a", "b"]`. `#` comments.
pub fn load() -> Vec<Rule> {
    parse(&std::fs::read_to_string(config_dir().join("rules")).unwrap_or_default())
}

pub fn parse(text: &str) -> Vec<Rule> {
    let mut rules: Vec<Rule> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line == "[[rule]]" {
            rules.push(Rule::default());
            continue;
        }
        // Anything before the first header, or without a `=`, is not ours.
        let (Some(rule), Some((key, value))) = (rules.last_mut(), line.split_once('=')) else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "extensions" => rule.extensions = list(value).iter().map(|e| e.trim_start_matches('.').to_lowercase()).collect(),
            "domains" => rule.domains = list(value).iter().map(|d| d.to_lowercase()).collect(),
            "min_size" => rule.min_size = bytes(&unquote(value)),
            "queue" => rule.queue = some(unquote(value)),
            "directory" => rule.directory = some(unquote(value)),
            "backend" => rule.backend = some(unquote(value)),
            _ => {}
        }
    }
    // A rule that decides nothing would silently swallow its matches.
    rules.retain(|r| r.queue.is_some() || r.directory.is_some() || r.backend.is_some());
    rules
}

/// The rules as a file, in the same subset of TOML `parse` reads. Written
/// whenever the panel changes one, so hand-editing and the panel agree.
pub fn render(rules: &[Rule]) -> String {
    let mut text = String::from("# muxget routing rules\n");
    for rule in rules {
        text.push_str("\n[[rule]]\n");
        for (i, name) in FIELDS.iter().enumerate() {
            let value = rule.get(i);
            if value.is_empty() {
                continue;
            }
            text.push_str(&match i {
                // Lists keep their brackets; everything else is one string.
                0 | 1 => format!("{name} = [{}]\n", quoted_list(&value)),
                _ => format!("{name} = \"{value}\"\n"),
            });
        }
    }
    text
}

pub fn save(rules: &[Rule]) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(config_dir().join("rules"), render(rules))
}

fn quoted_list(value: &str) -> String {
    value
        .split(',')
        .map(|v| format!("\"{}\"", v.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).to_string()
}

fn some(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(unquote)
        .filter(|s| !s.is_empty())
        .collect()
}
