use crate::library::actor::Actor;
use crate::library::actor::Subscriber;
use heapless::Vec;

const MAX_SUBSCRIBERS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureReadError {
    MeasurementFailed,
}

pub trait TemperaturePort {
    async fn read_temperature(&mut self) -> Result<f64, TemperatureReadError>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TemperatureReading {
    Measured(f64),
    MeasurementFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureSensorMessage {
    MeasureTemperature,
}

pub struct TemperatureActor<P> {
    port: P,
    subscribers: Vec<Subscriber<TemperatureReading>, MAX_SUBSCRIBERS>,
}

impl<P> TemperatureActor<P> {
    pub fn new(port: P) -> Self {
        Self { port, subscribers: Vec::new() }
    }

    pub fn subscribe(&mut self, subscriber: Subscriber<TemperatureReading>) -> Result<(), Subscriber<TemperatureReading>> {
        self.subscribers.push(subscriber)
    }
}

impl<P> Actor for TemperatureActor<P>
where
    P: TemperaturePort,
{
    type Message = TemperatureSensorMessage;

    async fn handle(&mut self, message: Self::Message) {
        let TemperatureSensorMessage::MeasureTemperature = message;
        let reading = match self.port.read_temperature().await {
            Ok(temperature) => TemperatureReading::Measured(temperature),
            Err(TemperatureReadError::MeasurementFailed) => TemperatureReading::MeasurementFailed,
        };
        for subscriber in &self.subscribers {
            subscriber.tell(reading);
        }
    }
}
