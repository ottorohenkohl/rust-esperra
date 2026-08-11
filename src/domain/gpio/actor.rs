use super::port::{LightPort, TemperatureSensorPort};
use super::protocol::{LightMessage, TemperatureSensorMessage, TemperatureSensorResponse};

use crate::library::actor::Actor;
use crate::library::actor::reference::Subscriber;
use crate::library::error::Error;

use embassy_time::Duration;
use heapless::Vec;

pub struct LightActor<P> {
    light_port: P,
    blink_duration: Duration,
}

impl<P> LightActor<P> {
    pub fn new(light_port: P, blink_duration: Duration) -> Self {
        Self { light_port, blink_duration }
    }
}

impl<P> Actor for LightActor<P>
where
    P: LightPort,
{
    type Message = LightMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => {
                let _ = self.light_port.set_active(active).await;
            }
            LightMessage::BlinkOnce => {
                let _ = self.light_port.blink(self.blink_duration).await;
            }
        }
    }
}

pub struct TemperatureActor<P> {
    temperature_port: P,
    subscribers: Vec<Subscriber<TemperatureSensorResponse>, 8>,
}

impl<P> TemperatureActor<P> {
    pub fn new(temperature_port: P) -> Self {
        Self { temperature_port, subscribers: Vec::new() }
    }

    pub fn subscribe(&mut self, subscriber: Subscriber<TemperatureSensorResponse>) -> Result<(), Subscriber<TemperatureSensorResponse>> {
        self.subscribers.push(subscriber)
    }
}

impl<P> Actor for TemperatureActor<P>
where
    P: TemperatureSensorPort,
{
    type Message = TemperatureSensorMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            TemperatureSensorMessage::MeasureTemperature => {
                let message = match self.temperature_port.read_temperature().await {
                    Ok(temperature) => TemperatureSensorResponse::Measured(temperature),
                    Err(Error) => TemperatureSensorResponse::MeasurementFailed,
                };

                self.subscribers.iter().for_each(|i| {
                    i.tell(message);
                });
            }
        }
    }
}
