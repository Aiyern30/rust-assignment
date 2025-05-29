use crate::common::data_types::ActuatorCommand;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let overheat_flag = Arc::new(Mutex::new(false));

        thread::spawn({
            let command_map = Arc::clone(&command_map);
            let overheat_flag = Arc::clone(&overheat_flag);

            move || {
                let mut next_instant = Instant::now();

                loop {
                    next_instant += interval;

                    let mut map = command_map.lock().unwrap();
                    for (actuator_id, queue) in map.iter_mut() {
                        queue.sort_by(|a, b| b.priority.cmp(&a.priority));

                        if let Some(command) = queue.pop() {
                            let cmd_type = &command.control_command.command_type;
                            let value = command.control_command.value;

                            if cmd_type == "RegulateTemperature" && value >= 80.0 {
                                *overheat_flag.lock().unwrap() = true;
                            } else if cmd_type == "RegulateTemperature" && value <= 30.0 {
                                *overheat_flag.lock().unwrap() = false;
                            }

                            let is_overheat = *overheat_flag.lock().unwrap();
                            if is_overheat && cmd_type != "RegulateTemperature" {
                                queue.insert(0, command);
                                continue;
                            }
                            if let Some(fwd_time) = command.forwarded_at {
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis();
                                let delay = now - fwd_time;
                                println!("⏱️ Scheduler delay for [{}]: {} ms", actuator_id, delay);
                            }

                            // Run the task
                            task_fn(actuator_id.clone(), command.clone());
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_millis();

                            let wait_time = now - command.forwarded_at.unwrap_or(0);
                            let deadline = if command.priority >= 10 { 1 } else { 2 };

                            if wait_time > deadline {
                                eprintln!(
                                    "⏱️ Actuator {} missed deadline ({}ms > {}ms)",
                                    actuator_id, wait_time, deadline
                                );
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
            }
        });
    }
}
