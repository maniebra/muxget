/// A named lane with its own concurrency limit. Downloads reference a queue by
/// `id`, so renaming or reordering queues never touches the downloads.
#[derive(Debug, Clone, PartialEq)]
pub struct Queue {
    pub id: usize,
    pub name: String,
    pub max_active: usize,
}

/// The queue every download lands in unless another is selected. Always exists.
pub const DEFAULT: usize = 0;

impl Queue {
    pub fn new(id: usize, name: &str, max_active: usize) -> Queue {
        Queue {
            id,
            name: name.trim().to_string(),
            max_active: max_active.clamp(1, 16),
        }
    }
}
