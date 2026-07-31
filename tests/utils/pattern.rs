use muxget::utils::{expand, fill, parse_range, MAX_EXPANSION};

#[test]
fn a_pattern_expands_over_its_range() {
    assert_eq!(
        expand("https://x.com/f%03d.iso", "1-3"),
        [
            "https://x.com/f001.iso",
            "https://x.com/f002.iso",
            "https://x.com/f003.iso"
        ]
    );
    assert_eq!(expand("https://x.com/f%d.iso", "9-10").len(), 2);
    // No range, or a broken one, is a single plain url.
    assert_eq!(expand("https://x.com/f.iso", ""), ["https://x.com/f.iso"]);
    assert_eq!(expand("https://x.com/f.iso", "10-1"), ["https://x.com/f.iso"]);
    assert_eq!(expand("https://x.com/f.iso", "a-b"), ["https://x.com/f.iso"]);
}

#[test]
fn a_huge_range_is_capped() {
    assert_eq!(expand("f%d", "1-100000").len(), MAX_EXPANSION as usize);
}

#[test]
fn only_the_number_specs_are_substituted() {
    assert_eq!(fill("a%db", 7), "a7b");
    assert_eq!(fill("a%04db", 7), "a0007b");
    assert_eq!(fill("100%% sure", 7), "100% sure");
    // An unknown spec is left as typed rather than guessed at.
    assert_eq!(fill("a%sb", 7), "a%sb");
    assert_eq!(fill("no specs", 7), "no specs");
}

#[test]
fn ranges_are_inclusive_and_non_negative() {
    assert_eq!(parse_range(" 2 - 5 "), Some((2, 5)));
    assert_eq!(parse_range("0-0"), Some((0, 0)));
    assert_eq!(parse_range("-1-5"), None);
    assert_eq!(parse_range("5"), None);
}
