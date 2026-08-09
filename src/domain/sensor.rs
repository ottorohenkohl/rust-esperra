use crate::domain::actor::ActorRef;
use crate::domain::boiler::BoilerMessage;

pub enum TemperatureSensorMessage {
    Measure,
}

pub trait TemperatureSensorActorRef: ActorRef<TemperatureSensorMessage> {}

impl<T: ActorRef<TemperatureSensorMessage>> TemperatureSensorActorRef for T {}

pub trait TemperatureReadingSubscriber: ActorRef<BoilerMessage> {}

impl<T: ActorRef<BoilerMessage>> TemperatureReadingSubscriber for T {}
