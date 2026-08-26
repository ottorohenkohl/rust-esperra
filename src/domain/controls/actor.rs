use super::protocol::CircuitMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::protocol::LightMessage;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::Actor;
use crate::library::actor::reference::ActorRef;

use embassy_time::{Duration, Instant};

const MAINTENANCE_GESTURE_MIN: Duration = Duration::from_secs(8);
const MAINTENANCE_GESTURE_MAX: Duration = Duration::from_secs(12);

pub struct CircuitActor<P, S, W> {
    actor_ref: ActorRef<CircuitMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_input_port: P,
    status_input_port: S,
    status_light_actor: ActorRef<LightMessage>,
    update_actor: ActorRef<UpdateMessage>,
    water_input_port: W,
    machine_powered: Option<bool>,
    power_off_since: Option<Instant>,
}

impl<P, S, W> CircuitActor<P, S, W>
where P: InputPort, S: InputPort, W: InputPort {
    pub fn new( actor_ref: ActorRef<CircuitMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, power_input_port: P, status_input_port: S, status_light_actor: ActorRef<LightMessage>, update_actor: ActorRef<UpdateMessage>, water_input_port: W) -> Self {
        let machine_powered = None;
        let power_off_since = None;

        Self { actor_ref, boiler_actor, brew_actor, power_input_port, status_input_port, status_light_actor, update_actor, water_input_port, machine_powered, power_off_since}
    }

    fn reached_update_interval(&mut self) -> bool {
        let off_duration = self.power_off_since.take().map(|time| Instant::now() - time);

        return matches!(off_duration, Some(duration) if duration >= MAINTENANCE_GESTURE_MIN && duration <= MAINTENANCE_GESTURE_MAX)
    }

    fn handle_power_input(&mut self) {
        let powered = self.power_input_port.powered_state();
        let previous = self.machine_powered.replace(powered);

        match (previous, powered) {
            (None, false) => {
                self.power_off_since = Some(Instant::now());

                self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive { active: false });
            }
            (Some(false), true) => {
                if self.reached_update_interval() {
                    return self.update_actor.tell(Some(&self.actor_ref), UpdateMessage::Start);
                }

                self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive { active: true });
            }
            _ => {}
        }
    }

    fn handle_status_input(&self) {
        let state = self.status_input_port.powered_state() && self.water_input_port.powered_state();

        self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(state));
    }

    fn handle_water_input(&self) {
        if self.water_input_port.powered_state() {
            return
        }

        self.status_light_actor.tell(Some(&self.actor_ref), LightMessage::BlinkTimes(1));
    }
}

impl<P, S, W> Actor for CircuitActor<P, S, W>
where P: InputPort, S: InputPort, W: InputPort {
    type Message = CircuitMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CircuitMessage::HandleInputStates => {
                self.handle_power_input();
                self.handle_status_input();
                self.handle_water_input();
            }
        }
    }
}
