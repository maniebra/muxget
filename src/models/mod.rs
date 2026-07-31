pub mod aria2;
pub mod crawl;
pub mod backend;
pub mod download;
pub mod option;
pub mod queue;
pub mod rule;
pub mod state;
pub mod wget;
pub mod ytdlp;

use backend::Backend;

/// Registry. Add a backend here and it is live everywhere.
pub fn backends() -> Vec<Box<dyn Backend>> {
    vec![
        Box::new(aria2::Aria2),
        Box::new(ytdlp::YtDlp),
        Box::new(wget::Wget::default()),
    ]
}

/// First backend that claims the url.
pub fn pick(url: &str) -> Option<Box<dyn Backend>> {
    backends().into_iter().find(|b| b.accepts(url))
}

/// A backend by name, for a routing rule that names one.
pub fn named(name: &str) -> Option<Box<dyn Backend>> {
    backends().into_iter().find(|b| b.name() == name)
}
