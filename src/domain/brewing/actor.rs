
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::port::OutputPort;
use crate::library::actor::Actor;

pub struct BrewActor<P, V> {
    pump_output_port: P,
    valve_output_port: V,
}

impl<P, V> BrewActor<P, V> where P: OutputPort, V: OutputPort {
    pub fn new(pump_output_port: P, valve_output_port: V) -> Self {
        Self { pump_output_port, valve_output_port }
    }
}

impl<P, V> Actor for BrewActor<P, V> where P: OutputPort, V: OutputPort {
    type Message = BrewMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BrewMessage::SetBrewing(active) => {
                self.pump_output_port.set_active(active);
                self.valve_output_port.set_active(active);
            }
            BrewMessage::SetPaused(active) => {
                self.pump_output_port.set_active(!active);
            }
        }
    }
}
