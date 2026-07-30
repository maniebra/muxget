pub mod aria2;
pub mod backend;
pub mod download;
pub mod queue;
pub mod ytdlp;

use backend::Backend;

/// Registry. Add a backend here and it is live everywhere.
pub fn backends() -> Vec<Box<dyn Backend>> {
    vec![Box::new(aria2::Aria2), Box::new(ytdlp::YtDlp)]
}

/// First backend that claims the url.
pub fn pick(url: &str) -> Option<Box<dyn Backend>> {
    backends().into_iter().find(|b| b.accepts(url))
}
