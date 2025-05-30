# Real-Time Sensor-Actuator System in Rust

## Overview

This project simulates a real-time sensor-actuator feedback system for an automated manufacturing line. Built using Rust, it models precision control where sensors continuously monitor environmental conditions and actuators respond dynamically to maintain system stability.

The architecture supports:

- Real-time communication between sensor and actuator components
- Anomaly detection and adaptive control using a PID algorithm
- Message-based inter-process communication with RabbitMQ
- Performance metrics tracking for benchmarking and optimization

---

## Project Structure

```text
rust-assignment/
├── benches/
│   └── criterion_main.rs        # Benchmarking entry point using Criterion
├── Cargo.toml                   # Project metadata and dependencies
├── Cargo.lock                   # Dependency lockfile
├── README.md                    # Project documentation (this file)
├── src/                         # Source code root
│
│   ├── config.rs                # System configuration setup
│   ├── lib.rs                   # Common library setup
│   ├── main.rs                  # Main CLI entry point (uses clap)
│
│   ├── actuator/                # Actuator control system (Student B)
│   │   ├── controller.rs        # PID controller logic
│   │   ├── executor.rs          # Executes actuator commands
│   │   ├── mod.rs               # Module declarations
│   │   ├── receiver.rs          # RabbitMQ receiver logic
│   │   ├── scheduler.rs         # Task scheduling and priority control
│   │   └── system.rs            # Main actuator runtime system
│
│   ├── bin/                     # Standalone binaries
│   │   ├── actuator.rs          # Runs actuator module standalone
│   │   └── sensor.rs            # Runs sensor module standalone
│
│   ├── common/                  # Shared utilities and types
│   │   ├── constants.rs         # Queue names, constants, flags
│   │   ├── data_types.rs        # Structs like SensorData, ActuatorCommand
│   │   ├── metrics.rs           # Performance tracking utilities
│   │   └── mod.rs               # Common module definitions
│
│   └── sensor/                  # Sensor module (Student A)
│       ├── generator.rs         # Generates simulated sensor data
│       ├── mod.rs               # Sensor module declarations
│       ├── processor.rs         # Filters and detects anomalies
│       └── transmitter.rs       # Publishes data to RabbitMQ
```

---

## How to Run

1.  Start RabbitMQ (Docker)

```
docker run -d --name rabbitmq -p 5672:5672 rabbitmq:3-management
```

2. Build the project

```
cargo build
```

3. Run the sensor system (in one terminal)

```
cargo run --bin sensor
```

4. Run the actuator system (in a second terminal)

```
cargo run --bin actuator
```

## Features

- Sensor data generation with realistic noise and drift

- Anomaly detection using threshold-based filtering

- Actuator control using PID feedback loops

- Asynchronous messaging with lapin and RabbitMQ

- Shared-state management with Arc<Mutex<T>>

- Performance benchmarking using Criterion.rs

## Authors

- Student A – Sensor Data Specialist

- Student B – Actuator Commander
