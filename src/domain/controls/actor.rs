use super::protocol::CircuitMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::protocol::LightMessage;
use crate::domain::update::port::UpdatePort;
use crate::library::actor::Actor;
use crate::library::actor::reference::ActorRef;

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};

const MAINTENANCE_GESTURE_MIN: Duration = Duration::from_secs(8);
const MAINTENANCE_GESTURE_MAX: Duration = Duration::from_secs(12);
const UPDATE_INDICATOR_INTERVAL: Duration = Duration::from_millis(500);

pub struct CircuitActor<P, S, W, U> {
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_input_port: P,
    status_input_port: S,
    power_light_actor: ActorRef<LightMessage>,
    status_light_actor: ActorRef<LightMessage>,
    water_input_port: W,
    update_port: Option<U>,
    machine_powered: Option<bool>,
    power_off_since: Option<Instant>,
}

impl<P, S, W, U> CircuitActor<P, S, W, U>
where
    P: InputPort,
    S: InputPort,
    W: InputPort,
    U: UpdatePort,
    U::Error: core::fmt::Debug,
{
    pub fn new(
        boiler_actor: ActorRef<BoilerMessage>,
        brew_actor: ActorRef<BrewMessage>,
        power_input_port: P,
        status_input_port: S,
        power_light_actor: ActorRef<LightMessage>,
        status_light_actor: ActorRef<LightMessage>,
        water_input_port: W,
        update_port: Option<U>,
    ) -> Self {
        Self {
            boiler_actor,
            brew_actor,
            power_input_port,
            status_input_port,
            power_light_actor,
            status_light_actor,
            water_input_port,
            update_port,
            machine_powered: None,
            power_off_since: None,
        }
    }

    fn handle_power_input(&mut self) -> bool {
        let powered = self.power_input_port.powered_state();
        let previous = self.machine_powered.replace(powered);

        match (previous, powered) {
            (None, true) => self.boiler_actor.tell(BoilerMessage::SetActive { active: true }),
            (Some(true), false) => {
                self.power_off_since = Some(Instant::now());
                self.boiler_actor.tell(BoilerMessage::SetActive { active: false });
            }
            (Some(false), true) => {
                let off_duration = self.power_off_since.take().map(|time| Instant::now() - time);
                if matches!(off_duration, Some(duration) if duration >= MAINTENANCE_GESTURE_MIN && duration <= MAINTENANCE_GESTURE_MAX) {
                    return true;
                }

                self.boiler_actor.tell(BoilerMessage::SetActive { active: true });
            }
            _ => {}
        }

        false
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

    fn start_update_indicator(&self) {
        self.power_light_actor.tell(LightMessage::BlinkTimes(1));
        self.status_light_actor.tell(LightMessage::BlinkTimes(1));
    }

    fn stop_update_indicator(&self) {
        self.power_light_actor.tell(LightMessage::SetActive(false));
        self.status_light_actor.tell(LightMessage::SetActive(false));
    }

    async fn blink_update_indicator(power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>) {
        loop {
            power_light_actor.tell(LightMessage::BlinkTimes(1));
            status_light_actor.tell(LightMessage::BlinkTimes(1));
            Timer::after(UPDATE_INDICATOR_INTERVAL).await;
        }
    }

    async fn handle_maintenance_gesture(&mut self) {
        self.boiler_actor.tell(BoilerMessage::SafetyShutdown);
        self.brew_actor.tell(BrewMessage::SetBrewing(false));

        if self.update_port.is_none() {
            self.boiler_actor.tell(BoilerMessage::SetActive { active: true });
            return;
        }

        self.start_update_indicator();
        match self.update_port.as_mut().unwrap().confirm() {
            Ok(true) => self.stop_update_indicator(),
            Ok(false) => {
                let power_light_actor = self.power_light_actor.clone();
                let status_light_actor = self.status_light_actor.clone();
                let result = {
                    let update = self.update_port.as_mut().unwrap().update();
                    select(update, Self::blink_update_indicator(power_light_actor, status_light_actor)).await
                };

                if let Either::First(Err(error)) = result {
                    log::warn!("update failed: {:?}", error);
                    self.stop_update_indicator();
                }
            }
            Err(error) => {
                log::warn!("image confirmation failed: {:?}", error);
                self.stop_update_indicator();
            }
        }
    }
}

impl<P, S, W, U> Actor for CircuitActor<P, S, W, U>
where
    P: InputPort,
    S: InputPort,
    W: InputPort,
    U: UpdatePort,
    U::Error: core::fmt::Debug,
{
    type Message = CircuitMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CircuitMessage::HandleInputStates => {
                if self.handle_power_input() {
                    self.handle_maintenance_gesture().await;
                    return;
                }

                self.handle_status_input();
                self.handle_water_input();
            }
        }
    }
}
