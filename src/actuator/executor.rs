use crate::common::data_types::ControlCommand;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn execute(&self, command: ControlCommand) {
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        println!(
            "[{}] Executing PID_OUTPUT command with value: {:.4}",
            start, command.value
        );

        std::thread::sleep(std::time::Duration::from_millis(5));

        let end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        println!("⏱️ Actuator execution time: {} ms", end - start);
    }
}
