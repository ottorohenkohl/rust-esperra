use super::protocol::CircuitMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::protocol::LightMessage;
use crate::library::actor::Actor;
use crate::library::actor::reference::ActorRef;

pub struct CircuitActor<P, S, W> {
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_input_port: P,
    status_input_port: S,
    status_light_actor: ActorRef<LightMessage>,
    water_input_port: W,
}

impl<P, S, W> CircuitActor<P, S, W> where P: InputPort, S: InputPort, W: InputPort {
    pub fn new(boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, power_input_port: P, status_input_port: S, status_light_actor: ActorRef<LightMessage>, water_input_port: W) -> Self {
        Self { boiler_actor, brew_actor, power_input_port, status_input_port, status_light_actor, water_input_port }
    }

    fn handle_power_input(&self) {
        let active = self.power_input_port.powered_state();

        self.boiler_actor.tell(BoilerMessage::SetActive { active });
    }

    fn handle_status_input(&self) {
        let state = self.status_input_port.powered_state() && self.water_input_port.powered_state();

        self.brew_actor.tell(BrewMessage::SetBrewing(state));
    }

    fn handle_water_input(&self) {
        if self.water_input_port.powered_state() {
            return
        }

        self.status_light_actor.tell(LightMessage::BlinkTimes(1));
    }
}

impl<P, S, W> Actor for CircuitActor<P, S, W> where P: InputPort, S: InputPort, W: InputPort {
    type Message = CircuitMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CircuitMessage::HandleInputStates  => {
                self.handle_power_input();
                self.handle_status_input();
                self.handle_water_input();
            }
        }
    }
}
