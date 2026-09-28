use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::port::OutputPort;
use crate::library::actor::interface::Actor;

pub struct BrewActor<P, V> {
    public_name: &'static str,

    pump_output_port: P,
    valve_output_port: V,
}

impl<P, V> BrewActor<P, V> where P: OutputPort, V: OutputPort {
    pub fn new(public_name: &'static str, pump_output_port: P, valve_output_port: V) -> Self {
        Self { public_name, pump_output_port, valve_output_port }
    }

    async fn set_brewing(&mut self, active: bool) {
        self.pump_output_port.set_active(active).await;
        self.valve_output_port.set_active(active).await;
    }
}

impl<P, V> Actor for BrewActor<P, V> where P: OutputPort, V: OutputPort {
    type Message = BrewMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BrewMessage::SetBrewing(active) => self.set_brewing(active).await,
        }
    }
}
