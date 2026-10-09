use super::port::{OutputPort, TemperatureSensorPort};
use super::protocol::{LightMode, LightingMessage, TemperatureSensorMessage, TemperatureSensorResponse};
use super::service::LightService;
use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::Subscriber;

use alloc::sync::Arc;
use heapless::Vec;


pub struct LightingActor<P, S> {
    public_name: &'static str,

    power_output_port: P,
    status_output_port: S,
    power: LightService,
    status: LightService,
    phase: bool,
}

impl<P, S> LightingActor<P, S> {
    pub fn new(public_name: &'static str, power_output_port: P, status_output_port: S) -> Self {
        let power = LightService::new();
        let status = LightService::new();

        let phase = true;

        Self { public_name, power_output_port, status_output_port, power, status, phase }
    }
}

impl<P, S> LightingActor<P, S> where P: OutputPort, S: OutputPort {
    fn set_power(&mut self, mode: LightMode) {
        self.power.set_mode(mode);
    }

    fn set_status(&mut self, mode: LightMode) {
        self.status.set_mode(mode);
    }

    fn set_both(&mut self, power: LightMode, status: LightMode) {
        self.power.set_mode(power);
        self.status.set_mode(status);
        self.power.cancel_blinks();
        self.status.cancel_blinks();
    }

    fn blink_status_times(&mut self, times: u8) {
        self.status.blink_times(times);
    }

    fn tick(&mut self) {
        self.phase ^= true;

        if self.phase {
            return
        }

        self.power.finish_pulse();
        self.status.finish_pulse();
    }

    async fn apply_outputs(&mut self) {
        self.power_output_port.set_active(self.power.active(self.phase)).await;
        self.status_output_port.set_active(self.status.active(self.phase)).await;
    }
}

impl<P, S> Actor for LightingActor<P, S> where P: OutputPort, S: OutputPort {
    type Message = LightingMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightingMessage::SetPower(mode) => self.set_power(mode),
            LightingMessage::SetStatus(mode) => self.set_status(mode),
            LightingMessage::SetBoth(power, status) => self.set_both(power, status),
            LightingMessage::BlinkStatusTimes(times) => self.blink_status_times(times),
            LightingMessage::Tick => self.tick(),
        }

        self.apply_outputs().await;
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
