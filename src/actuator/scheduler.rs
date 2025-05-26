use crate::common::data_types::ActuatorCommand;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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

    pub fn start<F>(
        &self,
        command_map: Arc<Mutex<HashMap<String, Vec<ActuatorCommand>>>>,
        mut task_fn: F,
    ) where
        F: FnMut(String, ActuatorCommand) + Send + 'static,
    {
        let interval = self.interval;
        thread::spawn(move || {
            let mut next_instant = Instant::now();
            loop {
                next_instant += interval;
                // task();
                {
                    let mut map = command_map.lock().unwrap();
                    for (actuator_id, queue) in map.iter_mut() {
                        if let Some(command) = queue.pop() {
                            // Run the task for this actuator's command
                            task_fn(actuator_id.clone(), command);
                        }
                    }
                }

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
