use crate::controllers::app::App;
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
        self.pump();
    }

    /// Switch the viewed queue by `delta` places.
    pub fn cycle_queue(&mut self, delta: isize) {
        let n = self.queues.len() as isize;
        self.current = (self.current as isize + delta).rem_euclid(n) as usize;
        self.clamp_selection();
    }
}
