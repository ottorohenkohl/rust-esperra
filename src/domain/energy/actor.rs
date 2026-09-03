use super::protocol::EnergyMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::gpio::protocol::LightMessage;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use embassy_time::{Duration, Instant};

const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60 * 60);

pub struct EnergyActor {
    actor_ref: ActorRef<EnergyMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    power_light_actor: ActorRef<LightMessage>,
    status_light_actor: ActorRef<LightMessage>,
    last_activity: Instant,
}

impl EnergyActor {
    pub fn new(actor_ref: ActorRef<EnergyMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>) -> Self {
        let last_activity = Instant::now();

        Self {
            actor_ref,
            boiler_actor,
            brew_actor,
            power_light_actor,
            status_light_actor,
            last_activity,
        }
    }

    fn set_active(&mut self, active: bool) {
        self.last_activity = Instant::now();

        if active {
            self.boiler_actor.unlock(&self.actor_ref);
            self.brew_actor.unlock(&self.actor_ref);
            self.power_light_actor.unlock(&self.actor_ref);
            self.status_light_actor.unlock(&self.actor_ref);
            self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));

            return
        }

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
        if Instant::now() - self.last_activity >= INACTIVITY_TIMEOUT {
            self.set_active(false);
        }
    }
}

impl Actor for EnergyActor {
    type Message = EnergyMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            EnergyMessage::SetActive(active) => self.set_active(active),
            EnergyMessage::CheckInactivity => self.check_inactivity(),
        }
    }
}
