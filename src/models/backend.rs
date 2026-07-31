use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::models::download::{Overrides, Progress, Status, Update};
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

    /// Flag this tool reads a config file with, and that file's contents for
    /// a login. Credentials go through a file so `ps` cannot show them.
    fn config_flag(&self) -> &'static str;
    fn credentials(&self, user: &str, pass: &str) -> String;
}

/// Spawn the backend and stream its progress to `tx` until it exits.
pub fn run(
    backend: Box<dyn Backend>,
    url: &str,
    dir: &Path,
    over: &Overrides,
    id: usize,
    tx: Sender<Update>,
) -> std::io::Result<Arc<Mutex<Child>>> {
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
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().expect("stdout piped above");
    let child = Arc::new(Mutex::new(child));
    let waiter = Arc::clone(&child);

    std::thread::spawn(move || {
        for_each_line(stdout, |line| {
            if let Some(p) = backend.parse(line) {
                let _ = tx.send(Update::Progress(id, p));
            } else if let Some(path) = destination(line) {
                let _ = tx.send(Update::Located(id, path));
            }
        });
        // ponytail: lock is only contended by cancel, which happens before EOF.
        let status = match waiter.lock().unwrap().wait() {
            Ok(s) if s.success() => Status::Done,
            Ok(s) => Status::Failed(format!("exit {}", s.code().unwrap_or(-1))),
            Err(e) => Status::Failed(e.to_string()),
        };
        clear_creds(id);
        let _ = tx.send(Update::Finished(id, status));
    });

    Ok(child)
}
