use super::protocol::{EnergyMessage, EnergyResponse};

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::protocol::LightMessage;
use crate::domain::logging::port::LoggingPort;
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
    power_light_actor: ActorRef<LightMessage>,
    status_light_actor: ActorRef<LightMessage>,
    logging_port: Arc<L>,
    last_activity: Option<Instant>,
}

impl<L> EnergyActor<L> where L: LoggingPort {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<EnergyMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>, logging_port: Arc<L>) -> Self {
        let last_activity = Some(Instant::now());

        Self { public_name, actor_ref, boiler_actor, brew_actor, power_light_actor, status_light_actor, logging_port, last_activity }
    }

    fn set_active(&mut self, active: bool) {
        if active {
            self.last_activity = Some(Instant::now());
            self.boiler_actor.unlock(&self.actor_ref);
            self.brew_actor.unlock(&self.actor_ref);
            self.power_light_actor.unlock(&self.actor_ref);
            self.status_light_actor.unlock(&self.actor_ref);
            self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));

            return
        }

        self.last_activity = None;
        self.boiler_actor.lock(&self.actor_ref);
        self.brew_actor.lock(&self.actor_ref);
        self.power_light_actor.lock(&self.actor_ref);
        self.status_light_actor.lock(&self.actor_ref);
        self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
        self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
        self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(false));
        self.status_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(false));
    }

    fn check_inactivity(&mut self) {
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
