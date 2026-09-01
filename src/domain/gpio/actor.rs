use super::port::{OutputPort, TemperatureSensorPort};
use super::protocol::{LightMessage, TemperatureSensorMessage, TemperatureSensorResponse};

use crate::domain::clock::port::TimerPort;
use crate::library::actor::Actor;
use crate::library::actor::reference::Subscriber;

use heapless::Vec;

pub struct LightActor<P, T> {
    output_port: P,
    timer_port: T,
}

impl<P, T> LightActor<P, T> {
    pub fn new(output_port: P, timer_port: T) -> Self {
        Self { output_port, timer_port }
    }
}

impl<P, T> Actor for LightActor<P, T> where P: OutputPort, T: TimerPort {
    type Message = LightMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => {
                let _ = self.output_port.set_active(active).await;
            }
            LightMessage::BlinkTimes(times) => {
                for _ in 0..(times * 2) {
                    self.output_port.invert_state().await;
                    self.timer_port.wait_for(200).await;
                }

                self.timer_port.wait_for(800).await;
            }
        }
    }
}

pub struct TemperatureSensorActor<P> {
    temperature_port: P,
    subscribers: Vec<Subscriber<TemperatureSensorResponse, 8>, 8>,
}

impl<P> TemperatureSensorActor<P> {
    pub fn new(temperature_port: P, subscribers: Vec<Subscriber<TemperatureSensorResponse, 8>, 8>) -> Self {
        Self { temperature_port, subscribers }
    }
}

impl<P> Actor for TemperatureSensorActor<P> where P: TemperatureSensorPort {
    type Message = TemperatureSensorMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            TemperatureSensorMessage::MeasureTemperature => {
                let message = match self.temperature_port.read_temperature().await {
                    Ok(temperature) => TemperatureSensorResponse::Measured(temperature),
                    Err(error) => TemperatureSensorResponse::MeasurementFailed(error),
                };

                self.subscribers.iter().for_each(|i| { i.tell(message) });
            }
        }
    }
}
