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
        thread::spawn(move || {
            let mut next_instant = Instant::now();
            loop {
                next_instant += Duration::from_millis(5);
                task();

                let now = Instant::now();
                if next_instant > now {
                    thread::sleep(next_instant - now);
                } else {
                    // Missed deadline, just continue without sleep
                    next_instant = now;
                }
            }
        });
    }
}
