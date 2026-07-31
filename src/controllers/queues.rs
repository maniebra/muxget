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

        for d in self.downloads.iter_mut().filter(|d| d.queue == id) {
            match (paused, &d.status) {
                (true, Status::Running) => d.pause(),
                // Resuming the queue resumes every paused row in it, including
                // ones paused by hand — one key, one predictable state.
                (false, Status::Paused) => d.resume(),
                _ => {}
            }
        }
        if !paused {
            self.pump();
        }
    }

    /// Give the current queue a daily active window, or clear it with an
    /// empty (or unparseable) value.
    pub fn set_schedule(&mut self, at: usize, text: &str) {
        let Some(queue) = self.queues.get_mut(at) else { return };
        queue.schedule = queue::parse_window(text);
        self.message = match self.queues[at].schedule {
            Some(_) => format!("{} runs {}", self.queues[at].name, self.queues[at].window()),
            None if text.trim().is_empty() => format!("{} schedule cleared", self.queues[at].name),
            None => "schedule must look like 22:00-06:00".into(),
        };
        self.save_state();
        self.apply_schedules();
    }

    /// Pause queues outside their window and resume them inside it. Manual
    /// pauses of an unscheduled queue are left alone — only a queue with a
    /// window is driven by the clock.
    pub fn apply_schedules(&mut self) {
        if !self.queues.iter().any(|q| q.schedule.is_some()) {
            return;
        }
        let Some(now) = queue::now_minutes() else { return };
        for at in 0..self.queues.len() {
            let q = &self.queues[at];
            if q.schedule.is_none() {
                continue;
            }
            let paused = !q.open_at(now);
            if paused != q.paused {
                self.set_queue_paused(at, paused);
            }
        }
    }

    /// Switch the viewed queue by `delta` places.
    pub fn cycle_queue(&mut self, delta: isize) {
        let n = self.queues.len() as isize;
        self.current = (self.current as isize + delta).rem_euclid(n) as usize;
        self.clamp_selection();
    }
}
