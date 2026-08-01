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

#[test]
fn a_pattern_captures_what_its_stars_cover() {
    use muxget::models::rule::capture;

    // The case that asked for this: one rule, a directory per channel.
    let caught = capture("youtube.com/@*", "https://youtube.com/@Fireship").unwrap();
    assert_eq!(caught, ["Fireship"]);
    // A star stops at the separator, so the rest of the url is not swallowed.
    let caught = capture("youtube.com/@*", "https://youtube.com/@Fireship/videos").unwrap();
    assert_eq!(caught, ["Fireship"]);
    // Case is ignored while matching and kept in what comes back.
    assert_eq!(capture("YOUTUBE.COM/@*", "https://youtube.com/@MIT").unwrap(), ["MIT"]);

    // Several stars are numbered left to right.
    let caught = capture("://*/*/releases/", "https://gh.io/rust-lang/releases/1.0").unwrap();
    assert_eq!(caught, ["gh.io", "rust-lang"]);

    // A pattern that is not in the url matches nothing.
    assert!(capture("youtube.com/@*", "https://vimeo.com/12345").is_none());
    // Neither does one whose star would have to cross a separator.
    assert!(capture("example.com/*.iso", "https://example.com/a/b.iso").is_none());
}

#[test]
fn a_capture_fills_the_destination_in() {
    use muxget::models::rule::Rule;

    let mut rule = Rule::default();
    rule.set(2, "youtube.com/@*");
    rule.set(5, "/home/mani/yt/$1");
    rule.set(4, "$1");

    let url = "https://youtube.com/@Fireship/videos";
    assert!(rule.matches(url), "a rule may be nothing but a pattern");
    let caught = rule.captures(url).unwrap();
    assert_eq!(rule.fill(&rule.get(5), &caught).unwrap(), "/home/mani/yt/Fireship");
    assert_eq!(rule.fill(&rule.get(4), &caught).unwrap(), "Fireship", "queues take one too");

    // A `$1` the pattern cannot fill is refused rather than taken literally —
    // it would otherwise become a directory named `$1`.
    let mut half = Rule::default();
    half.set(1, "youtube.com");
    half.set(5, "/home/mani/yt/$1");
    assert!(half.matches(url), "the rule still applies");
    assert_eq!(half.fill("/home/mani/yt/$1", &half.captures(url).unwrap()), None);
    // And so is a $2 when the pattern has only one star.
    assert_eq!(rule.fill("/home/mani/yt/$1/$2", &caught), None);

    assert!(!rule.matches("https://youtube.com/watch?v=abc"), "no channel, no match");
    // And the rule survives the file.
    assert_eq!(muxget::models::rule::parse(&muxget::models::rule::render(&[rule.clone()])), [rule]);
}
