use std::time::{Duration, Instant};

use crate::controllers::app::App;
use crate::models::download::Status;
use crate::models::queue::{self, Queue};

/// Queue CRUD and slot limits.
impl App {
    /// The queue currently being viewed. Always valid — `current` is clamped
    /// on every change and the default queue can never be removed.
    pub fn queue(&self) -> &Queue {
        &self.queues[self.current.min(self.queues.len() - 1)]
    }

    /// Change how many downloads the current queue may run at once.
    pub fn set_max_active(&mut self, n: usize) {
        let i = self.current;
        self.queues[i].max_active = n.clamp(1, 16);
        self.save_state();
        self.message = format!(
            "{}: {} concurrent",
            self.queues[i].name, self.queues[i].max_active
        );
        self.pump();
    }

    /// Create a queue and switch to it. Blank or duplicate names are refused.
    pub fn add_queue(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if self.queues.iter().any(|q| q.name == name) {
            self.message = format!("queue {name} already exists");
            return;
        }
        let id = self.next_queue_id;
        self.next_queue_id += 1;
        self.queues.push(Queue::new(id, name, 3));
        self.current = self.queues.len() - 1;
        self.clamp_selection();
        self.message = format!("queue {name} created");
        self.save_state();
    }

    pub fn rename_queue(&mut self, at: usize, name: &str) {
        let name = name.trim();
        if name.is_empty() || at >= self.queues.len() {
            return;
        }
        if self.queues.iter().enumerate().any(|(i, q)| i != at && q.name == name) {
            self.message = format!("queue {name} already exists");
            return;
        }
        self.queues[at].name = name.to_string();
        self.save_state();
    }

    /// Remove a queue; its downloads move to the default queue rather than
    /// being silently killed. The default queue itself cannot be removed.
    pub fn delete_queue(&mut self, at: usize) {
        let Some(q) = self.queues.get(at) else { return };
        if q.id == queue::DEFAULT {
            self.message = "the default queue cannot be deleted".into();
            return;
        }
        let (id, name) = (q.id, q.name.clone());
        for d in self.downloads.iter_mut().filter(|d| d.queue == id) {
            d.queue = queue::DEFAULT;
        }
        self.queues.remove(at);
        self.current = self.current.min(self.queues.len() - 1);
        self.clamp_selection();
        self.message = format!("queue {name} deleted, downloads moved to default");
        self.save_state();
        self.pump();
    }

    /// Pause the current queue, or resume it if already paused.
    pub fn toggle_queue_pause(&mut self) {
        let at = self.current;
        let paused = !self.queues[at].paused;
        self.set_queue_paused(at, paused);
        self.message = format!(
            "queue {} {}",
            self.queues[at].name,
            if paused { "paused" } else { "resumed" }
        );
    }

    /// Pause every queue, or resume them all if any is paused. Resuming wins
    /// so a half-paused app reaches a known state in one keypress.
    pub fn toggle_all_pause(&mut self) {
        let paused = !self.queues.iter().any(|q| q.paused);
        for at in 0..self.queues.len() {
            self.set_queue_paused(at, paused);
        }
        self.message = if paused {
            "all queues paused".into()
        } else {
            "all queues resumed".into()
        };
    }

    /// A paused queue holds its running downloads frozen and starts no new
    /// ones; resuming reverses both halves.
    fn set_queue_paused(&mut self, at: usize, paused: bool) {
        let Some(queue) = self.queues.get_mut(at) else { return };
        queue.paused = paused;
        let id = queue.id;

        let rows: Vec<usize> = self
            .downloads
            .iter()
            .enumerate()
            .filter(|(_, d)| d.queue == id)
            .map(|(at, _)| at)
            .collect();
        for at in rows {
            match (paused, &self.downloads[at].status) {
                (true, Status::Running) => self.downloads[at].pause(),
                // Resuming the queue resumes every paused row in it, including
                // ones paused by hand — one key, one predictable state.
                (false, Status::Paused) => self.resume_row(at),
                _ => {}
            }
        }
        if !paused {
            self.pump();
        }
    }

    /// Set the queue's schedule from a typed spec; empty clears it.
    pub fn set_schedule(&mut self, at: usize, text: &str) {
        let Some(queue) = self.queues.get_mut(at) else { return };
        let ok = queue.set_spec(text);
        self.message = match (ok, text.trim().is_empty()) {
            (_, true) => format!("{} schedule cleared", self.queues[at].name),
            (true, _) => format!("{} runs {}", self.queues[at].name, self.queues[at].window()),
            (false, _) => "e.g. 22:00-06:00 mon-fri retry=3 quota=150MB/4h".into(),
        };
        self.save_state();
        self.apply_schedules();
    }

    /// The upkeep pass, run every few seconds: window and quota pauses,
    /// periodic re-sync, and the actions a drained queue triggers. A queue
    /// with no schedule at all is never paused here, so hand pauses survive.
    pub fn apply_schedules(&mut self) {
        let now = queue::now();
        for at in 0..self.queues.len() {
            self.roll_quota(at);
            self.sync_queue(at);
            if let Some(now) = &now {
                let q = &self.queues[at];
                if q.scheduled() {
                    let paused = !q.open_now(now);
                    if paused != q.paused {
                        self.set_queue_paused(at, paused);
                    }
                }
            }
            self.on_drained(at);
        }
    }

    /// Start a new quota period once the old one is up, which un-pauses a
    /// queue that spent its allowance.
    fn roll_quota(&mut self, at: usize) {
        let q = &mut self.queues[at];
        let Some((_, minutes)) = q.quota else { return };
        if q.since.elapsed() < Duration::from_secs(minutes as u64 * 60) {
            return;
        }
        q.since = Instant::now();
        q.used = 0;
    }

    /// Charge a queue for what it has moved since the last tick. Reported
    /// speed integrated over the tick, so the count drifts a few percent.
    /// speed × elapsed, exact byte counters if a quota needs to be tight.
    pub fn charge_quotas(&mut self, secs: f64) {
        for at in 0..self.queues.len() {
            if self.queues[at].quota.is_none() {
                continue;
            }
            let moved = self.speed_in(self.queues[at].id) * secs;
            self.queues[at].used += moved as u64;
        }
    }

    /// Periodic synchronisation: requeue everything finished, so the queue
    /// re-fetches it. The interval is wall time since the app started.
    fn sync_queue(&mut self, at: usize) {
        let q = &self.queues[at];
        let Some(minutes) = q.sync else { return };
        if q.synced.elapsed() < Duration::from_secs(minutes as u64 * 60) {
            return;
        }
        self.queues[at].synced = Instant::now();
        let id = self.queues[at].id;
        let mut found = 0;
        for d in self.downloads.iter_mut().filter(|d| d.queue == id) {
            if matches!(d.status, Status::Done | Status::Failed(_) | Status::Cancelled) {
                d.status = Status::Queued;
                d.tries = 0;
                found += 1;
            }
        }
        if found > 0 {
            self.message = format!("{}: re-syncing {found} downloads", self.queues[at].name);
            self.pump();
        }
    }

    /// What a queue does once nothing in it is left to run: the `after`
    /// command, the shutdown, the `once` teardown. Each fires once per drain.
    fn on_drained(&mut self, at: usize) {
        let q = &self.queues[at];
        if !(q.once || q.shutdown || !q.after.is_empty()) {
            return;
        }
        let id = q.id;
        let busy = self.downloads.iter().any(|d| {
            d.queue == id && matches!(d.status, Status::Running | Status::Queued | Status::Paused)
        });
        // Nothing to do until it has actually had work and finished it.
        let worked = self.downloads.iter().any(|d| d.queue == id);
        if busy || !worked {
            self.queues[at].fired = false;
            return;
        }
        if std::mem::replace(&mut self.queues[at].fired, true) {
            return;
        }

        let q = &self.queues[at];
        let (name, after, shutdown, once) =
            (q.name.clone(), q.after.clone(), q.shutdown, q.once);
        if !after.is_empty() {
            self.message = match std::process::Command::new("sh").arg("-c").arg(&after).spawn() {
                Ok(_) => format!("{name} finished, ran {after}"),
                Err(e) => format!("{name}: could not run {after}: {e}"),
            };
        }
        if once {
            // A one-shot run is over; drop the schedule so a restart or the
            // next midnight does not start it again.
            self.queues[at].set_spec("");
            self.set_queue_paused(at, true);
            self.message = format!("{name} finished its one-time run");
            self.save_state();
        }
        if shutdown {
            self.message = format!("{name} finished, shutting down");
            let _ = std::process::Command::new("shutdown").arg("-h").arg("now").spawn();
        }
    }

    /// Move the current queue `delta` places in the tab order. Downloads
    /// reference queues by id, so reordering never moves a download.
    pub fn move_queue(&mut self, delta: isize) {
        let Some(to) = self
            .current
            .checked_add_signed(delta)
            .filter(|to| *to < self.queues.len())
        else {
            return;
        };
        self.queues.swap(self.current, to);
        self.current = to;
        self.save_state();
    }

    /// Switch the viewed queue by `delta` places.
    pub fn cycle_queue(&mut self, delta: isize) {
        let n = self.queues.len() as isize;
        self.current = (self.current as isize + delta).rem_euclid(n) as usize;
        self.clamp_selection();
    }
}
