use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use crate::models::backend::Backend;
use crate::models::download::{Overrides, Progress};
use crate::utils::{args, parse};

/// The crawler's backend. wget already recurses, de-duplicates, maps url paths
/// to directories, rewrites links and skips unchanged files, so a crawl is a
/// wget invocation rather than a crawler of our own.
#[derive(Default)]
pub struct Wget {
    /// The url of the request wget is reporting on; its errors name the
    /// failure a line or two after the url they belong to.
    at: Mutex<String>,
    /// Files this run left alone because the server's copy was no newer.
    skipped: Mutex<u32>,
}

impl Backend for Wget {
    fn name(&self) -> &'static str {
        "wget"
    }

    /// Never claims a url on its own — a crawl or a rule asks for it by name,
    /// and aria2c is the better fetcher for a plain file.
    fn accepts(&self, _url: &str) -> bool {
        false
    }

    fn command(&self, url: &str, dir: &Path, over: &Overrides) -> Command {
        let mut c = Command::new("wget");
        // Dots are the only progress format wget writes to a log rather than
        // a terminal, which is where a piped run sends it.
        c.arg("--progress=dot:mega")
            .arg("--directory-prefix")
            .arg(if over.dir.is_empty() { dir } else { Path::new(&over.dir) });
        c.args(args::load(self.name()));
        if !over.rate.is_empty() {
            c.arg(format!("--limit-rate={}", over.rate));
        }
        if !over.name.is_empty() {
            c.arg("--output-document").arg(&over.name);
        }
        // The crawl's own flags: recursion depth, filters, mirror mode.
        c.args(args::parse(&over.args));
        // wget logs to stderr; the shared spawner reads stdout.
        c.arg("--output-file").arg("-").arg(url);
        c
    }

    fn parse(&self, line: &str) -> Option<Progress> {
        if let Some(url) = line.split_once("--  ").map(|(_, u)| u.trim()) {
            if url.starts_with("http") {
                *self.at.lock().ok()? = url.to_string();
            }
        }
        parse::wget(line)
    }

    /// What a crawl has to say for itself: resources it could not fetch, and
    /// the tally at the end, including what a re-crawl left alone.
    fn notice(&self, line: &str) -> Option<String> {
        let at = || self.at.lock().map(|u| u.clone()).unwrap_or_default();
        // Both wordings mean the same: the local copy is already current.
        if line.contains("Omitting download") || line.contains("-- not retrieving") {
            let mut skipped = self.skipped.lock().ok()?;
            *skipped += 1;
            return Some(format!("{skipped} unchanged files skipped"));
        }
        if let Some((_, reason)) = line.split_once(" ERROR ") {
            return Some(format!("missing {}: {}", at(), reason.trim()));
        }
        if line.contains("broken link") || line.contains("unable to resolve") {
            return Some(format!("missing {}", at()));
        }
        let done = line.trim().strip_prefix("Downloaded: ")?;
        Some(match *self.skipped.lock().ok()? {
            0 => format!("crawl finished: {done}"),
            n => format!("crawl finished: {done}, {n} unchanged files skipped"),
        })
    }

    /// Some of the site's resources were not there. Every one of them was
    /// reported as it happened, and the rest of the copy is good.
    fn tolerates(&self, code: i32) -> bool {
        code == 8
    }

    /// wget's documented exit codes.
    fn reason(&self, code: i32) -> String {
        match code {
            1 => "wget was called wrong".into(),
            2 => "bad wget config".into(),
            3 => "could not write the file".into(),
            4 => "network problem".into(),
            5 => "certificate problem".into(),
            6 => "authorization failed".into(),
            7 => "protocol error".into(),
            8 => "the server refused some of it".into(),
            code => format!("exit {code}"),
        }
    }

    fn config_flag(&self) -> &'static str {
        "--config"
    }

    fn credentials(&self, user: &str, pass: &str) -> String {
        format!("user = {user}\npassword = {pass}\n")
    }
}
