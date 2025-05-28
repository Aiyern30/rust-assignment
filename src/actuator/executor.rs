use crate::common::data_types::ControlCommand;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    // pub fn execute(&self, command: ControlCommand) {
    //     println!(
    //         "[{}] Executing {} command with value: {:.4}",
    //         command.timestamp, command.command_type, command.value
    //     );

    //     if let Some(payload) = &command.payload {
    //         println!("Payload: {}", payload);
    //     }
    // }
    pub fn execute(&self, command: ControlCommand) {
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        println!(
            "[{}] Executing PID_OUTPUT command with value: {:.4}",
            start, command.value
        );

        // Simulate actuator processing
        std::thread::sleep(std::time::Duration::from_millis(5)); // if needed

        let end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        println!("⏱️ Actuator execution time: {} ms", end - start);
    }
}
