use super::protocol::{EnergyMessage, EnergyResponse};

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::protocol::{LightMode, LightingMessage};
use crate::domain::logging::port::LoggingPort;
use crate::domain::weight::protocol::WeightMessage;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;
use embassy_time::{Duration, Instant};

const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60 * 60);

pub struct EnergyActor<L> {
    public_name: &'static str,

    actor_ref: ActorRef<EnergyMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    lighting_actor: ActorRef<LightingMessage>,
    weight_actor: ActorRef<WeightMessage>,
    logging_port: Arc<L>,
    last_activity: Option<Instant>,
}

impl<L> EnergyActor<L> where L: LoggingPort {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<EnergyMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, lighting_actor: ActorRef<LightingMessage>, weight_actor: ActorRef<WeightMessage>, logging_port: Arc<L>) -> Self {
        let last_activity = Some(Instant::now());

        Self { public_name, actor_ref, boiler_actor, brew_actor, lighting_actor, weight_actor, logging_port, last_activity }
    }

    fn set_active(&mut self, active: bool) {
        if active {
            self.last_activity = Some(Instant::now());

            self.boiler_actor.unlock(&self.actor_ref);
            self.brew_actor.unlock(&self.actor_ref);
            self.lighting_actor.unlock(&self.actor_ref);

            self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));

            return
        }

        self.last_activity = None;

        self.boiler_actor.lock(&self.actor_ref);
        self.brew_actor.lock(&self.actor_ref);
        self.lighting_actor.lock(&self.actor_ref);

        self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
        self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
        self.weight_actor.tell(Some(&self.actor_ref), WeightMessage::Shutdown);

        self.switch_lights_off();
    }

    fn switch_lights_off(&self) {
        self.lighting_actor.tell(Some(&self.actor_ref), LightingMessage::SetBoth(LightMode::Off, LightMode::Off));
    }

    fn check_inactivity(&mut self) {
        if self.last_activity.is_none() {
            self.switch_lights_off();

            return
        }

        if matches!(self.last_activity, Some(last_activity) if Instant::now() - last_activity >= INACTIVITY_TIMEOUT) {
            self.set_active(false);
        }
    }

    fn log_active_state(&self) {
        self.logging_port.log(self, None, &EnergyResponse::Active(self.last_activity.is_some()));
    }
}

impl<L> Actor for EnergyActor<L> where L: LoggingPort {
    type Message = EnergyMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            EnergyMessage::SetActive(active) => self.set_active(active),
            EnergyMessage::CheckInactivity => self.check_inactivity(),
            EnergyMessage::LogActiveState => self.log_active_state(),
        }
    }
}
