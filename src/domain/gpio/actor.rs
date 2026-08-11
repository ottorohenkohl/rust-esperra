use super::port::{LightPort, TemperatureSensorPort};
use super::protocol::{LightMessage, TemperatureSensorMessage, TemperatureSensorResponse};

use crate::library::actor::Actor;
use crate::library::actor::reference::Subscriber;
use crate::library::error::Error;

use heapless::Vec;

pub struct LightActor<P> {
    light_port: P,
}

impl<P> LightActor<P> {
    pub fn new(light_port: P) -> Self {
        Self { light_port }
    }
}

impl<P> Actor for LightActor<P> where P: LightPort {
    type Message = LightMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => {
                let _ = self.light_port.set_active(active).await;
            }
            LightMessage::BlinkTimes(times) => {
                let _ = self.light_port.blink(times).await;
            }
        }
    }
}

pub struct TemperatureSensorActor<P> {
    temperature_port: P,
    subscribers: Vec<Subscriber<TemperatureSensorResponse>, 8>,
}

impl<P> TemperatureSensorActor<P> {
    pub fn new(temperature_port: P) -> Self {
        let subscribers = Vec::new();

        Self { temperature_port, subscribers }
    }

    pub fn subscribe(&mut self, subscriber: Subscriber<TemperatureSensorResponse>) -> Result<(), Subscriber<TemperatureSensorResponse>>
    {
        self.subscribers.push(subscriber)
    }
}

impl<P> Actor for TemperatureSensorActor<P> where P: TemperatureSensorPort {
    type Message = TemperatureSensorMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            TemperatureSensorMessage::MeasureTemperature => {
                let message = match self.temperature_port.read_temperature().await {
                    Ok(temperature) => TemperatureSensorResponse::Measured(temperature),
                    Err(Error) => TemperatureSensorResponse::MeasurementFailed,
                };

                self.subscribers.iter().for_each(|i| { i.tell(message) });
            }
        }
    }
}
