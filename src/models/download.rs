#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent: f32,
    pub speed: String,
    pub eta: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Accepted, waiting for a free slot.
    Queued,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Download {
    /// Stable across removals — worker threads report against this, not an index.
    pub id: usize,
    pub url: String,
    pub backend: &'static str,
    pub status: Status,
    pub progress: Progress,
    pub child: Option<std::sync::Arc<std::sync::Mutex<std::process::Child>>>,
}

impl Download {
    pub fn kill(&mut self) {
        if let Some(child) = self.child.take() {
            let _ = child.lock().unwrap().kill();
        }
    }
}

/// What a worker thread reports back to the controller.
pub enum Update {
    Progress(usize, Progress),
    Finished(usize, Status),
}
