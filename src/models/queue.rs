/// A named lane with its own concurrency limit. Downloads reference a queue by
/// `id`, so renaming or reordering queues never touches the downloads.
#[derive(Debug, Clone, PartialEq)]
pub struct Queue {
    pub id: usize,
    pub name: String,
    pub max_active: usize,
    /// While set, nothing in this queue starts — `pump` skips it entirely.
    pub paused: bool,
    /// Daily active window as local minutes-of-day, `(start, end)`. Inside it
    /// the queue runs, outside it the queue is paused; `None` means always on.
    pub schedule: Option<(u16, u16)>,
}

/// The queue every download lands in unless another is selected. Always exists.
pub const DEFAULT: usize = 0;

impl Queue {
    pub fn new(id: usize, name: &str, max_active: usize) -> Queue {
        Queue {
            id,
            name: name.trim().to_string(),
            max_active: max_active.clamp(1, 16),
            paused: false,
            schedule: None,
        }
    }

    /// True when `now` (local minutes-of-day) is in the window. One whose end
    /// is not after its start wraps past midnight (`22:00-06:00`).
    pub fn open_at(&self, now: u16) -> bool {
        match self.schedule {
            None => true,
            Some((start, end)) if start <= end => (start..end).contains(&now),
            Some((start, end)) => now >= start || now < end,
        }
    }

    /// `HH:MM-HH:MM` for the file and the UI; empty when there is no window.
    pub fn window(&self) -> String {
        match self.schedule {
            None => String::new(),
            Some((s, e)) => format!("{}-{}", clock(s), clock(e)),
        }
    }
}

fn clock(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// `HH:MM-HH:MM`; empty or malformed clears the window rather than guessing.
pub fn parse_window(text: &str) -> Option<(u16, u16)> {
    let (start, end) = text.trim().split_once('-')?;
    let at = |s: &str| {
        let (h, m) = s.trim().split_once(':')?;
        let (h, m): (u16, u16) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
        (h < 24 && m < 60).then_some(h * 60 + m)
    };
    let (start, end) = (at(start)?, at(end)?);
    // An empty window would pause the queue forever.
    (start != end).then_some((start, end))
}

/// Local minutes-of-day, via `date(1)` rather than a time crate. `None`
/// leaves every schedule open.
pub fn now_minutes() -> Option<u16> {
    let out = std::process::Command::new("date").arg("+%H:%M").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}
