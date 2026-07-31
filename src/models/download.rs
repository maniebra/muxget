#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent: f32,
    pub speed: String,
    pub eta: String,
}

/// Per-download tweaks typed in the add form. An empty field means "use the
/// app-wide setting"; each backend maps the rest to its own flags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    /// Download directory for this item only.
    pub dir: String,
    /// Output file name for this item only.
    pub name: String,
    /// Speed cap, in whatever the backend accepts (`2M`, `500K`).
    pub rate: String,
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
    /// Settings that apply to this download alone.
    pub over: Overrides,
    pub status: Status,
    pub progress: Progress,
    /// Where the file landed, once a backend names it. Not persisted — a
    /// restarted transfer names it again.
    pub path: Option<std::path::PathBuf>,
    pub child: Option<std::sync::Arc<std::sync::Mutex<std::process::Child>>>,
}

impl Download {
    pub fn kill(&mut self) {
        // A stopped process still dies on SIGKILL, so no need to resume first.
        if let Some(child) = self.child.take() {
            let _ = child.lock().unwrap().kill();
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

    /// `kill(1)` rather than a libc dependency for two signals.
    fn signal(&self, sig: &str) -> bool {
        let Some(child) = &self.child else {
            // No process (a test row, or one that already exited).
            return true;
        };
        let pid = child.lock().unwrap().id();
        std::process::Command::new("kill")
            .arg(sig)
            .arg(pid.to_string())
            .status()
            .is_ok_and(|s| s.success())
    }
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
