#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub percent: f32,
    pub speed: String,
    pub eta: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Running,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Download {
    pub url: String,
    pub backend: &'static str,
    pub status: Status,
    pub progress: Progress,
}

/// What a worker thread reports back to the controller.
pub enum Update {
    Progress(usize, Progress),
    Finished(usize, Status),
}
