use crate::common::constants::{ACTUATOR_COMMAND_QUEUE, ACTUATOR_FEEDBACK_QUEUE};
use crate::common::data_types::{ActuatorCommand, ActuatorFeedback};
use crossbeam_channel::{Receiver, Sender};
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde_json;

#[allow(dead_code)]
pub async fn run_transmitter(
    command_rx: Receiver<ActuatorCommand>,
    feedback_tx: Sender<ActuatorFeedback>,
) -> anyhow::Result<()> {
    let conn =
        Connection::connect("amqp://127.0.0.1:5672/%2f", ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    channel
        .queue_declare(
            ACTUATOR_COMMAND_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            ACTUATOR_FEEDBACK_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let feedback_channel = channel.clone();
    let tx_clone = feedback_tx.clone();
    tokio::spawn(async move {
        use futures::StreamExt;
        let mut consumer = feedback_channel
            .basic_consume(
                ACTUATOR_FEEDBACK_QUEUE,
                "sensor_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();

        while let Some(delivery) = consumer.next().await {
            if let Ok(delivery) = delivery {
                if let Ok(feedback) = serde_json::from_slice::<ActuatorFeedback>(&delivery.data) {
                    tx_clone.send(feedback).ok();
                }
                delivery.ack(BasicAckOptions::default()).await.unwrap();
            }
        }
    });

    while let Ok(command) = command_rx.recv() {
        let data = serde_json::to_vec(&command)?;
        channel
            .basic_publish(
                "",
                ACTUATOR_COMMAND_QUEUE,
                BasicPublishOptions::default(),
                &data,
                BasicProperties::default(),
            )
            .await?
            .await?;
    }

    Ok(())
}
