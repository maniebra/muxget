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
