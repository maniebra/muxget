use std::process::Stdio;

use crate::controllers::app::App;
use crate::controllers::keys::{Dialog, Form};
use crate::models::crawl::{self, Crawl, Found};
use crate::models::download::{Overrides, Update};
use crate::utils::args;
use crate::utils::parse::{for_each_line, human};

/// The crawl form's fields, in display order.
pub const CRAWL_LABELS: [&str; 7] = [
    "url",
    "depth",
    "extensions",
    "include",
    "exclude",
    "size min-max",
    "options",
];

/// The crawl the form opens with: the settings panel's crawler tab, or the
/// built-in defaults where it says nothing.
pub fn defaults() -> Crawl {
    let saved = args::to_pairs(&args::load("crawl"));
    let set = |flag: &str| saved.iter().any(|(f, _)| f == flag);
    let value = |flag: &str| {
        saved.iter().find(|(f, _)| f == flag).map(|(_, v)| v.clone()).unwrap_or_default()
    };
    let base = Crawl::default();
    let (min, max) = split_size(&value("size"));
    Crawl {
        depth: value("depth").trim().parse().unwrap_or(base.depth),
        exts: list(&value("extensions")),
        min_size: min,
        max_size: max,
        same_domain: !set("any-domain"),
        under_path: set("under-path"),
        ignore_robots: set("no-robots"),
        flat: set("flat"),
        ..base
    }
}

/// Comma-separated extensions, without their dots.
fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|p| p.trim().trim_start_matches('.').to_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

fn split_size(text: &str) -> (Option<f64>, Option<f64>) {
    let (min, max) = text.split_once('-').unwrap_or((text, ""));
    (crate::utils::parse::bytes(min), crate::utils::parse::bytes(max))
}

/// Build a crawl from what was typed. A field left empty keeps the default
/// from the crawler tab, and anything unparseable falls back to it too.
pub fn from_form(form: &Form) -> Crawl {
    let options = form.fields[6].to_lowercase();
    let base = defaults();
    // A word turns its setting on, and the opposite word turns it back off,
    // so one crawl can go against the saved default either way.
    let word = |on: &str, off: &str, default: bool| match (
        options.split_whitespace().any(|w| w == on),
        options.split_whitespace().any(|w| w == off),
    ) {
        (true, false) => true,
        (false, true) => false,
        _ => default,
    };
    let (min, max) = match form.fields[5].trim().is_empty() {
        true => (base.min_size, base.max_size),
        false => split_size(&form.fields[5]),
    };
    Crawl {
        url: form.fields[0].trim().to_string(),
        depth: form.fields[1].trim().parse().unwrap_or(base.depth),
        exts: match form.fields[2].trim().is_empty() {
            true => base.exts.clone(),
            false => list(&form.fields[2]),
        },
        // Patterns are matched as typed; only extensions are dot-stripped.
        include: form.fields[3].split(',').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect(),
        exclude: form.fields[4].split(',').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect(),
        min_size: min,
        max_size: max,
        same_domain: !word("any-domain", "same-domain", !base.same_domain),
        under_path: word("under-path", "any-path", base.under_path),
        flat: word("flat", "nested", base.flat),
        // `ignore-robots` is what people type when they do not remember
        // which of the two it is.
        ignore_robots: match options.contains("ignore-robots") {
            true => true,
            false => word("no-robots", "robots", base.ignore_robots),
        },
        // A mirror is a decision about this crawl, not a saved default.
        offline: options.contains("offline"),
    }
}

impl App {
    /// Start a crawl: either mirror the site for offline reading, or walk it
    /// and come back with the list of links to pick from.
    pub fn start_crawl(&mut self, crawl: Crawl) {
        if crawl.url.trim().is_empty() {
            return;
        }
        if crawl.offline {
            self.mirror(&crawl);
            return;
        }
        self.message = format!("crawling {} …", crawl.url);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            // wget logs to stderr, and the spider writes nothing else.
            let child = crawl
                .spider_command()
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Update::Notice(format!("wget: {e}")));
                    return;
                }
            };
            let stderr = child.stderr.take().expect("piped above");
            let mut found: Vec<Found> = Vec::new();
            for_each_line(stderr, |line| crawl::collect(line, &mut found));
            let _ = child.wait();

            // The page itself is what was typed, not a discovery.
            found.retain(|f| f.url != crawl.url && !crawl::is_plumbing(&f.url) && crawl.keep(f));
            let _ = tx.send(Update::Crawled(crawl, found));
        });
    }

    /// Download the whole site as one row: wget mirrors it, rewrites the
    /// links, and on a re-run fetches only what changed.
    fn mirror(&mut self, crawl: &Crawl) {
        let queue = self.queue().id;
        let over = Overrides {
            args: crawl.mirror_args().join(" "),
            backend: "wget".into(),
            ..Default::default()
        };
        self.enqueue(&crawl.url, queue, over);
        self.message = format!("mirroring {} for offline use", crawl.url);
    }

    /// Discovery finished: show what it found, or say that it found nothing.
    pub(crate) fn crawled(&mut self, crawl: Crawl, found: Vec<Found>) {
        if found.is_empty() {
            self.message = format!("nothing matched under {}", crawl.url);
            return;
        }
        let total: f64 = found.iter().filter_map(|f| f.size).sum();
        self.message = format!("{} links, {}", found.len(), human(total));
        self.dialog = Some(Dialog::Crawled(Box::new(crawl), found, Vec::new(), 0));
    }

    /// Queue the picked links, each under the local path its url maps to.
    pub fn add_found(&mut self, crawl: &Crawl, found: &[Found], picked: &[usize]) {
        let queue = self.queue().id;
        let base = self.dir.clone();
        for f in picked.iter().filter_map(|i| found.get(*i)) {
            let dir = crawl::local_dir(&f.url, crawl.flat);
            let over = Overrides {
                dir: match dir.is_empty() {
                    true => String::new(),
                    false => base.join(dir).display().to_string(),
                },
                // Backends name the file themselves; only step in when the
                // url would not give a usable one.
                name: match needs_name(&f.url) {
                    true => crawl::local_name(&f.url),
                    false => String::new(),
                },
                ..Default::default()
            };
            self.enqueue(&f.url, queue, over);
        }
        self.message = format!("queued {} of {} links", picked.len(), found.len());
    }
}

/// A url whose last segment would make a poor file name: a query string, or
/// no segment at all.
fn needs_name(url: &str) -> bool {
    let path = url.split(['#']).next().unwrap_or(url);
    path.contains('?') || path.rsplit('/').next().is_none_or(|last| last.is_empty())
}
