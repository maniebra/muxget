use std::time::Instant;

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
    /// Weekdays the window applies to, bit 0 = Monday. `0` means every day.
    pub days: u8,
    /// Single calendar date, `YYYY-MM-DD`; the queue runs on that day only.
    pub date: Option<String>,
    /// Run through once, then pause and drop the schedule.
    pub once: bool,
    /// Requeue everything finished this often, in minutes — IDM's periodic
    /// synchronisation.
    pub sync: Option<u32>,
    /// How often a failed download in this queue is retried before it sticks.
    pub retry: u8,
    /// Shell command run once the queue drains.
    pub after: String,
    /// Shut the machine down once the queue drains.
    pub shutdown: bool,
    /// `(bytes, minutes)` — at most this much traffic per period.
    pub quota: Option<(u64, u32)>,
    /// The schedule as typed; the file and the dialog both use this text.
    pub spec: String,

    /// Bytes spent in the current quota period, and when that period began.
    /// Both reset with the period, and neither survives a restart.
    pub used: u64,
    pub since: Instant,
    /// Last periodic sync, and whether the drain actions already fired.
    pub synced: Instant,
    pub fired: bool,
}

/// The queue every download lands in unless another is selected. Always exists.
pub const DEFAULT: usize = 0;

/// Local time as the schedule sees it: minutes-of-day, ISO weekday (1 = Monday)
/// and calendar date.
pub struct Now {
    pub minutes: u16,
    pub weekday: u8,
    pub date: String,
}

impl Queue {
    pub fn new(id: usize, name: &str, max_active: usize) -> Queue {
        Queue {
            id,
            name: name.trim().to_string(),
            max_active: max_active.clamp(1, 16),
            paused: false,
            schedule: None,
            days: 0,
            date: None,
            once: false,
            sync: None,
            retry: 0,
            after: String::new(),
            shutdown: false,
            quota: None,
            spec: String::new(),
            used: 0,
            since: Instant::now(),
            synced: Instant::now(),
            fired: false,
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

    /// The full test: window, weekdays, date and quota. A queue with no
    /// schedule at all is always open.
    pub fn open_now(&self, now: &Now) -> bool {
        if let Some(date) = &self.date {
            if *date != now.date {
                return false;
            }
        }
        if self.days != 0 && self.days & (1 << (now.weekday.saturating_sub(1))) == 0 {
            return false;
        }
        if let Some((bytes, _)) = self.quota {
            if self.used >= bytes {
                return false;
            }
        }
        self.open_at(now.minutes)
    }

    /// True when this queue is scheduled at all, so hand pauses on an
    /// unscheduled queue are never overridden.
    pub fn scheduled(&self) -> bool {
        self.schedule.is_some()
            || self.date.is_some()
            || self.quota.is_some()
            || self.days != 0
    }

    /// The schedule as typed, for the file and the UI; empty when there is none.
    pub fn window(&self) -> String {
        self.spec.clone()
    }

    /// Apply a typed spec, replacing whatever was there. Empty clears it.
    pub fn set_spec(&mut self, text: &str) -> bool {
        // A new spec replaces the old one whole; leftovers from the previous
        // one (a stale weekday mask, a spent quota) would be invisible.
        let paused = self.paused;
        *self = Queue { paused, ..Queue::new(self.id, &self.name, self.max_active) };
        parse_spec(text, self)
    }
}

const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

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

/// `1h`, `30m`, `2d` → minutes.
fn parse_every(text: &str) -> Option<u32> {
    let text = text.trim();
    let (n, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    let n: u32 = n.parse().ok()?;
    Some(match unit {
        "m" => n,
        "h" => n * 60,
        "d" => n * 1440,
        _ => return None,
    })
    .filter(|m| *m > 0)
}

/// `150MB`, `2G`, `900K` → bytes.
fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let (n, unit) = text.split_at(split);
    let n: f64 = n.parse().ok()?;
    let scale = match unit.to_ascii_lowercase().trim_end_matches('b') {
        "k" => 1024.0,
        "m" => 1024.0 * 1024.0,
        "g" => 1024.0 * 1024.0 * 1024.0,
        "" => 1.0,
        _ => return None,
    };
    Some((n * scale) as u64).filter(|b| *b > 0)
}

/// `mon`, `mon-fri`, `sat,sun` → a weekday bitmask; `0` when nothing parses.
fn parse_days(text: &str) -> u8 {
    let index = |name: &str| DAYS.iter().position(|d| *d == name);
    let mut mask = 0u8;
    for part in text.split(',') {
        match part.split_once('-') {
            Some((from, to)) => {
                let (Some(from), Some(to)) = (index(from), index(to)) else { continue };
                // A range that wraps the week (`sat-tue`) still means both ends.
                let mut i = from;
                loop {
                    mask |= 1 << i;
                    if i == to {
                        break;
                    }
                    i = (i + 1) % 7;
                }
            }
            None => {
                if let Some(i) = index(part) {
                    mask |= 1 << i;
                }
            }
        }
    }
    mask
}

/// The full schedule line, whitespace separated, any order:
///
/// `22:00-06:00 mon-fri once on=2026-08-01 sync=6h retry=3 quota=150MB/4h`
/// `shutdown after=<command, rest of the line>`
///
/// Returns false when something in it did not parse; whatever did parse is
/// still applied, so one typo does not silently drop the rest.
pub fn parse_spec(text: &str, q: &mut Queue) -> bool {
    // `after=` swallows the rest, so a command may contain spaces and `=`.
    let (head, after) = match text.split_once("after=") {
        Some((head, cmd)) => (head, cmd.trim()),
        None => (text, ""),
    };
    q.after = after.replace('|', " ");
    let mut ok = true;

    for token in head.split_whitespace() {
        let (key, value) = token.split_once('=').unwrap_or((token, ""));
        match key {
            "once" => q.once = true,
            "shutdown" => q.shutdown = true,
            "on" => match value.split('-').count() == 3 && value.len() == 10 {
                true => q.date = Some(value.to_string()),
                false => ok = false,
            },
            "sync" => match parse_every(value) {
                Some(m) => q.sync = Some(m),
                None => ok = false,
            },
            "retry" => match value.parse() {
                Ok(n) => q.retry = n,
                Err(_) => ok = false,
            },
            "quota" => match value
                .split_once('/')
                .and_then(|(size, every)| Some((parse_size(size)?, parse_every(every)?)))
            {
                Some(quota) => q.quota = Some(quota),
                None => ok = false,
            },
            _ if token.contains(':') => match parse_window(token) {
                Some(w) => q.schedule = Some(w),
                None => ok = false,
            },
            _ => match parse_days(token) {
                0 => ok = false,
                mask => q.days |= mask,
            },
        }
    }

    // Store it normalised, so the file never carries a `|` into a queue line.
    q.spec = text.trim().replace('|', " ");
    ok
}

/// Local time, via `date(1)` rather than a time crate. `None` leaves every
/// schedule open.
pub fn now() -> Option<Now> {
    let out = std::process::Command::new("date")
        .arg("+%H:%M %u %F")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let (h, m) = parts.next()?.split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    let weekday: u8 = parts.next()?.parse().ok()?;
    let date = parts.next()?.to_string();
    (h < 24 && m < 60 && (1..=7).contains(&weekday)).then_some(Now {
        minutes: h * 60 + m,
        weekday,
        date,
    })
}

/// Local minutes-of-day. `None` leaves every schedule open.
pub fn now_minutes() -> Option<u16> {
    now().map(|n| n.minutes)
}
