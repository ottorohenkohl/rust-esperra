use super::implementation::MqttMessagingImpl;

use embassy_executor::task;

#[task]
pub async fn messaging_task(implementation: MqttMessagingImpl) {
    implementation.run().await;
}
