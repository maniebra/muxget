use muxget::models::rule::{parse, Rule};

const RULES: &str = r#"
# Big images go to their own queue and folder.
[[rule]]
extensions = ["iso", "img"]
queue = "large-files"
directory = "~/Downloads/ISOs"

[[rule]]
domains = ["youtube.com", "youtu.be"]
queue = "media"

[[rule]]
min_size = "5G"
queue = "overnight"

# Decides nothing, so it is dropped rather than swallowing its matches.
[[rule]]
extensions = ["zip"]
"#;

fn rules() -> Vec<Rule> {
    parse(RULES)
}

#[test]
fn rules_parse_in_order() {
    let rules = rules();
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].extensions, ["iso", "img"]);
    assert_eq!(rules[0].queue.as_deref(), Some("large-files"));
    assert_eq!(rules[0].directory.as_deref(), Some("~/Downloads/ISOs"));
    assert_eq!(rules[1].domains, ["youtube.com", "youtu.be"]);
    assert_eq!(rules[2].min_size, Some(5.0 * 1024.0 * 1024.0 * 1024.0));
}

#[test]
fn a_url_matches_on_extension_and_domain() {
    let rules = rules();
    assert!(rules[0].matches("https://example.com/a/arch.ISO"));
    assert!(rules[0].matches("https://example.com/disk.img?token=1"));
    assert!(!rules[0].matches("https://example.com/a.zip"));

    assert!(rules[1].matches("https://www.youtube.com/watch?v=abc"));
    assert!(rules[1].matches("https://youtu.be/abc"));
    assert!(!rules[1].matches("https://vimeo.com/1"));

    // A size-only rule matches any url and waits for the total.
    assert!(rules[2].matches("https://example.com/whatever"));
    assert!(!rules[2].wants_size(1.0));
    assert!(rules[2].wants_size(6.0 * 1024.0 * 1024.0 * 1024.0));
}

#[test]
fn every_set_condition_has_to_match() {
    let both = parse("[[rule]]\nextensions = [\"iso\"]\ndomains = [\"mirror.net\"]\nqueue = \"q\"\n");
    assert!(both[0].matches("https://mirror.net/a.iso"));
    assert!(!both[0].matches("https://elsewhere.net/a.iso"));
    assert!(!both[0].matches("https://mirror.net/a.zip"));
}
