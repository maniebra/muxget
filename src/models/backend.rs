use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use crate::models::download::{Overrides, Progress, Status, Update};
use crate::models::log;
use crate::utils::parse::{destination, for_each_line};
use crate::utils::{clear_creds, write_creds};

/// A download tool. Supply a command line and a progress parser; the spawning,
/// line reading and reporting below is shared by every implementation.
pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;

    /// Does this backend want to handle `url`?
    fn accepts(&self, url: &str) -> bool;

    /// Command to run. Must write progress to stdout.
    fn command(&self, url: &str, dir: &Path, over: &Overrides) -> Command;

    /// One line of tool output -> progress, or None if the line says nothing.
    fn parse(&self, line: &str) -> Option<Progress>;

    /// Something worth telling the user that is not progress — a resource a
    /// crawl could not fetch, the tally at the end of one.
    fn notice(&self, _line: &str) -> Option<String> {
        None
    }

    /// What a non-zero exit code means, for tools that document theirs.
    fn reason(&self, code: i32) -> String {
        format!("exit {code}")
    }

    /// A non-zero exit this backend does not consider a failure — a crawl
    /// that could not fetch one of a thousand images still did its job, and
    /// the ones it missed were reported as they happened.
    fn tolerates(&self, _code: i32) -> bool {
        false
    }

    /// Flag this tool reads a config file with, and that file's contents for
    /// a login. Credentials go through a file so `ps` cannot show them.
    fn config_flag(&self) -> &'static str;
    fn credentials(&self, user: &str, pass: &str) -> String;
}

/// A command as it would be typed, for the log. Arguments are as given: a
/// password never reaches one — it goes through a config file — so there is
/// nothing here to redact.
fn printable(cmd: &Command) -> String {
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    format!("{} {}", cmd.get_program().to_string_lossy(), args.join(" "))
}

/// Spawn the backend and stream its progress to `tx` until it exits.
pub fn run(
    backend: Box<dyn Backend>,
    url: &str,
    dir: &Path,
    over: &Overrides,
    id: usize,
    tx: Sender<Update>,
) -> std::io::Result<u32> {
    let mut cmd = backend.command(url, dir, over);
    if !over.user.is_empty() || !over.pass.is_empty() {
        match write_creds(id, &backend.credentials(&over.user, &over.pass)) {
            Some(path) => {
                cmd.arg(backend.config_flag()).arg(path);
            }
            // Better than starting silently unauthenticated.
            None => {
                return Err(std::io::Error::other("could not store credentials"));
            }
        }
    }
    log::info(format!("[{id}] {}", printable(&cmd)));
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .inspect_err(|e| log::error(format!("[{id}] {} would not start: {e}", backend.name())))?;
    let stdout = child.stdout.take().expect("stdout piped above");
    let stderr = child.stderr.take().expect("stderr piped above");
    let pid = child.id();

    // Whatever the tool complains about, kept for the log tab. This is where
    // the reason for a failure actually lives — the exit code only says that
    // there was one.
    std::thread::spawn(move || {
        for_each_line(stderr, |line| log::warn(format!("[{id}] {line}")));
    });

    std::thread::spawn(move || {
        for_each_line(stdout, |line| {
            if let Some(p) = backend.parse(line) {
                let _ = tx.send(Update::Progress(id, p));
            } else if let Some(path) = destination(line) {
                let _ = tx.send(Update::Located(id, path));
            } else if let Some(note) = backend.notice(line) {
                let _ = tx.send(Update::Notice(note));
            }
        });
        let status = match child.wait() {
            Ok(s) if s.success() => Status::Done,
            Ok(s) if s.code().is_some_and(|c| backend.tolerates(c)) => Status::Done,
            Ok(s) => Status::Failed(match s.code() {
                Some(code) => backend.reason(code),
                None => "killed".to_string(),
            }),
            Err(e) => Status::Failed(e.to_string()),
        };
        clear_creds(id);
        match &status {
            Status::Failed(reason) => log::error(format!("[{id}] failed: {reason}")),
            _ => log::info(format!("[{id}] finished")),
        }
        let _ = tx.send(Update::Finished(id, status));
    });

    Ok(pid)
}
