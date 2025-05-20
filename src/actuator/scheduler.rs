use std::thread;
use std::time::{Duration, Instant};

pub struct Scheduler {
    interval: Duration,
}

impl Scheduler {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval: Duration::from_millis(interval_ms),
        }
    }

    pub fn start<F>(&self, mut task: F)
    where
        F: FnMut() + Send + 'static,
    {
        let interval = self.interval;
        thread::spawn(move || {
            let mut next_instant = Instant::now();
            loop {
                next_instant += interval;
                task();

                let now = Instant::now();
                if next_instant > now {
                    thread::sleep(next_instant - now);
                } else {
                    next_instant = now;
                }
            }
        });
    }
}
