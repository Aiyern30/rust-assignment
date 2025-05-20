use crate::common::data_types::ControlCommand;

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn execute(&self, command: ControlCommand) {
        println!(
            "[{}] Executing {} command with value: {:.4}",
            command.timestamp, command.command_type, command.value
        );

        if let Some(payload) = &command.payload {
            println!("Payload: {}", payload);
        }
    }
}
