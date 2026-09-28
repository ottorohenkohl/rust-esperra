use super::port::{OutputPort, TemperatureSensorPort};
use super::protocol::{LightMessage, LightResponse, TemperatureSensorMessage, TemperatureSensorResponse};

use crate::domain::clock::port::TimerPort;
use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::Subscriber;

use alloc::sync::Arc;
use heapless::Vec;

pub struct LightActor<P, T, L> {
    public_name: &'static str,

    output_port: P,
    timer_port: T,
    logging_port: Arc<L>,
}

impl<P, T, L> LightActor<P, T, L> {
    pub fn new(public_name: &'static str, output_port: P, timer_port: T, logging_port: Arc<L>) -> Self {
        Self { output_port, timer_port, public_name, logging_port }
    }
}

impl<P, T, L> LightActor<P, T, L> where P: OutputPort, T: TimerPort, L: LoggingPort {
    async fn set_active(&mut self, active: bool) {
        self.output_port.set_active(active).await;
        self.logging_port.log(self, None, &LightResponse::Active(active));
    }

    async fn blink_times(&mut self, times: u8) {
        for _ in 0..(times * 2) {
            let active = self.output_port.invert_state().await;

            self.logging_port.log(self, None, &LightResponse::Active(active));
            self.timer_port.wait_for(500).await;
        }

        self.timer_port.wait_for(1000).await;
    }
}

impl<P, T, L> Actor for LightActor<P, T, L> where P: OutputPort, T: TimerPort, L: LoggingPort {
    type Message = LightMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => self.set_active(active).await,
            LightMessage::BlinkTimes(times) => self.blink_times(times).await,
        }
    }
}

pub struct TemperatureSensorActor<P, L> {
    public_name: &'static str,

    temperature_port: P,
    logging_port: Arc<L>,
    subscribers: Vec<Subscriber<TemperatureSensorResponse, 8>, 8>,
}

impl<P, L> TemperatureSensorActor<P, L> {
    pub fn new(public_name: &'static str, temperature_port: P, logging_port: Arc<L>, subscribers: Vec<Subscriber<TemperatureSensorResponse, 8>, 8>) -> Self {
        Self { public_name, temperature_port, logging_port, subscribers }
    }
}

impl<P, L> TemperatureSensorActor<P, L> where P: TemperatureSensorPort, L: LoggingPort {
    async fn measure_temperature(&mut self) {
        let message = match self.temperature_port.read_temperature().await {
            Ok(temperature) => {
                let message = TemperatureSensorResponse::Measured(temperature);

                self.logging_port.log(self, None, &message);

                message
            },
            Err(error) => TemperatureSensorResponse::MeasurementFailed(error),
        };

        self.subscribers.iter().for_each(|subscriber| subscriber.tell(message));
    }
}

impl<P, L> Actor for TemperatureSensorActor<P, L> where P: TemperatureSensorPort, L: LoggingPort {
    type Message = TemperatureSensorMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            TemperatureSensorMessage::MeasureTemperature => self.measure_temperature().await,
        }
    }
}
