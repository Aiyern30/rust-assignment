use crate::common::data_types::ControlCommand;

pub struct Executor;

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn execute(&self, command: ControlCommand) {
        println!("Executing control command with value: {}", command.value);
        // TODO: Add real actuator interface here
    }
}
