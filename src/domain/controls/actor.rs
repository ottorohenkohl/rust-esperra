use super::protocol::{ButtonMessage, WaterMessage, WaterResponse};

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::protocol::LightMessage;
use crate::domain::logging::port::LoggingPort;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;
use embassy_time::{Duration, Instant};

const MAINTENANCE_GESTURE_MIN: Duration = Duration::from_secs(8);
const MAINTENANCE_GESTURE_MAX: Duration = Duration::from_secs(12);

pub struct ButtonActor<P, S> {
    public_name: &'static str,
    actor_ref: ActorRef<ButtonMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_input_port: P,
    status_input_port: S,
    update_actor: ActorRef<UpdateMessage>,
    energy_actor: ActorRef<EnergyMessage>,
    machine_powered: Option<bool>,
    power_off_since: Option<Instant>,
}

impl<P, S> ButtonActor<P, S> where P: InputPort, S: InputPort {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<ButtonMessage>, brew_actor: ActorRef<BrewMessage>, power_input_port: P, status_input_port: S, update_actor: ActorRef<UpdateMessage>, energy_actor: ActorRef<EnergyMessage>) -> Self {
        let machine_powered = None;
        let power_off_since = None;

        Self { public_name, actor_ref, brew_actor, power_input_port, status_input_port, update_actor, energy_actor, machine_powered, power_off_since }
    }

    fn reached_update_interval(&mut self) -> bool {
        let off_duration = self.power_off_since.take().map(|time| Instant::now() - time);

        matches!(off_duration, Some(duration) if duration >= MAINTENANCE_GESTURE_MIN && duration <= MAINTENANCE_GESTURE_MAX)
    }

    fn handle_power_input(&mut self) {
        let powered = self.power_input_port.powered_state();
        let previous = self.machine_powered.replace(powered);

        match (previous, powered) {
            (Some(true), false) | (None, false) => {
                self.power_off_since = Some(Instant::now());

                self.energy_actor.tell(Some(&self.actor_ref), EnergyMessage::SetActive(false));
            }
            (Some(false), true) | (None, true) => {
                if self.reached_update_interval() {
                    return self.update_actor.tell(Some(&self.actor_ref), UpdateMessage::Start);
                }

                self.energy_actor.tell(Some(&self.actor_ref), EnergyMessage::SetActive(true));
            }
            _ => {}
        }
    }

    fn handle_status_input(&self) {
        let active = self.status_input_port.powered_state();

        if active {
            self.energy_actor.tell(Some(&self.actor_ref), EnergyMessage::SetActive(true));
        }

        self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(active));
    }
}

impl<P, S> Actor for ButtonActor<P, S> where P: InputPort, S: InputPort {
    type Message = ButtonMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            ButtonMessage::HandleInputStates => {
                self.handle_power_input();
                self.handle_status_input();
            }
        }
    }
}

pub struct WaterActor<W, L> {
    actor_ref: ActorRef<WaterMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_light_actor: ActorRef<LightMessage>,
    water_input_port: W,
    public_name: &'static str,
    logging_port: Arc<L>,
}

impl<W, L> WaterActor<W, L> where W: InputPort, L: LoggingPort {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<WaterMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, power_light_actor: ActorRef<LightMessage>, water_input_port: W, logging_port: Arc<L>) -> Self {
        Self { actor_ref, boiler_actor, brew_actor, power_light_actor, water_input_port, public_name, logging_port }
    }

    fn handle_water_input(&self) {
        let enough_water = self.water_input_port.powered_state();

        self.logging_port.log(self, None, &WaterResponse::EnoughWater(enough_water));

        if enough_water {
            self.boiler_actor.unlock(&self.actor_ref);
            self.brew_actor.unlock(&self.actor_ref);

            self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(true));

            return
        }

        self.boiler_actor.lock(&self.actor_ref);
        self.brew_actor.lock(&self.actor_ref);

        self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
        self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
        self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::BlinkTimes(1));
    }
}

impl<W, L> Actor for WaterActor<W, L> where W: InputPort, L: LoggingPort {
    type Message = WaterMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            WaterMessage::HandleInputState => {
                self.handle_water_input();
            }
        }
    }
}
