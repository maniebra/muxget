#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent: f32,
    pub speed: String,
    pub eta: String,
    /// Bytes done and expected, as the backend wrote them.
    pub done: String,
    pub total: String,
    /// Upload rate, session total, connected peers and seeders. Only aria2
    /// torrents report these; `seeders` staying `None` marks a row as not one.
    pub upload: String,
    pub uploaded: String,
    pub peers: u32,
    pub seeders: Option<u32>,
}

impl Progress {
    pub fn is_torrent(&self) -> bool {
        self.seeders.is_some()
    }

    /// Peers that are not seeding, so still downloading themselves.
    pub fn leechers(&self) -> u32 {
        self.peers.saturating_sub(self.seeders.unwrap_or(0))
    }
}

/// Per-download settings from the add form. Empty means "use the app-wide
/// setting"; each backend maps the rest to its own flags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub dir: String,
    pub name: String,
    /// Speed cap, in whatever the backend accepts (`2M`, `500K`).
    pub rate: String,
    /// Backend to use instead of the one the url would pick, from a rule.
    pub backend: String,
    /// The password is never persisted and never reaches a command line.
    pub user: String,
    pub pass: String,
}

impl Overrides {
    pub fn is_empty(&self) -> bool {
        *self == Overrides::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Accepted, waiting for a free slot.
    Queued,
    Running,
    /// Process stopped with SIGSTOP; its slot is free until it resumes.
    Paused,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Download {
    /// Stable across removals — worker threads report against this, not an index.
    pub id: usize,
    /// Queue this download belongs to (`queue::Queue::id`).
    pub queue: usize,
    pub url: String,
    pub backend: &'static str,
    pub over: Overrides,
    pub status: Status,
    pub progress: Progress,
    /// Where the file landed, once a backend names it. Not persisted.
    pub path: Option<std::path::PathBuf>,
    /// The backend process, while it runs. A pid rather than the `Child`:
    /// waiting on a child holds it, and killing through the same lock is how
    /// a quit hangs and leaves the download orphaned.
    pub pid: Option<u32>,
    /// Failed attempts so far, against the queue's retry limit. A restart is
    /// a fresh start, so this is not persisted.
    pub tries: u8,
}

impl Download {
    pub fn kill(&mut self) {
        // A stopped process still dies on SIGKILL, so no need to resume first.
        if let Some(pid) = self.pid.take() {
            signal(pid, "-KILL");
        }
    }

    /// Freeze the transfer, keeping the process, its connections and its
    /// partial file. Long pauses may still have the server drop the socket;
    /// both backends retry, and resume with `--continue` either way.
    pub fn pause(&mut self) {
        if self.signal("-STOP") {
            self.status = Status::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.signal("-CONT") {
            self.status = Status::Running;
        }
    }

    fn signal(&self, sig: &str) -> bool {
        match self.pid {
            // No process (a test row, or one that already exited).
            None => true,
            Some(pid) => signal(pid, sig),
        }
    }
}

/// `kill(1)` rather than a libc dependency for three signals.
pub fn signal(pid: u32, sig: &str) -> bool {
    std::process::Command::new("kill")
        .arg(sig)
        .arg(pid.to_string())
        .status()
        .is_ok_and(|s| s.success())
}

/// What a worker thread reports back to the controller.
pub enum Update {
    Progress(usize, Progress),
    /// The output path a backend just named.
    Located(usize, String),
    Finished(usize, Status),
    /// A playlist entry found by the expander: (queue id, url, overrides).
    Discovered(usize, String, Overrides),
    /// Status line for the footer.
    Notice(String),
}
