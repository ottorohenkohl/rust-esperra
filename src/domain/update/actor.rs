use super::port::{UpdateConfirmation, UpdatePort};
use super::protocol::UpdateMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::controls::protocol::{ButtonMessage, WaterMessage};
use crate::domain::gpio::protocol::{LightMessage, TemperatureSensorMessage};
use crate::library::actor::{Actor, ActorRef};

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};

const SAFE_SHUTDOWN_DELAY: Duration = Duration::from_secs(2);
const UPDATE_INDICATOR_INTERVAL: Duration = Duration::from_millis(500);

pub struct UpdateActor<U: UpdatePort> {
    actor_ref: ActorRef<UpdateMessage>,
    update_port: U,
    button_actor: ActorRef<ButtonMessage>,
    water_actor: ActorRef<WaterMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    temperature_sensor_actor: ActorRef<TemperatureSensorMessage>,
    power_light_actor: ActorRef<LightMessage>,
    status_light_actor: ActorRef<LightMessage>,
}

impl<U: UpdatePort> UpdateActor<U> {
    pub fn new(actor_ref: ActorRef<UpdateMessage>, update_port: U, button_actor: ActorRef<ButtonMessage>, water_actor: ActorRef<WaterMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, temperature_sensor_actor: ActorRef<TemperatureSensorMessage>, power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>) -> Self {
        Self { actor_ref, update_port, button_actor, water_actor, boiler_actor, brew_actor, temperature_sensor_actor, power_light_actor, status_light_actor }
    }

    fn acquire_locks(&self) -> bool {
        let button_actor_locked = self.button_actor.lock(&self.actor_ref);
        let water_actor_locked = self.water_actor.lock(&self.actor_ref);
        let boiler_actor_locked = self.boiler_actor.lock(&self.actor_ref);
        let brew_actor_locked = self.brew_actor.lock(&self.actor_ref);
        let temperature_sensor_actor_locked = self.temperature_sensor_actor.lock(&self.actor_ref);
        let power_light_actor_locked = self.power_light_actor.lock(&self.actor_ref);
        let status_light_actor_locked = self.status_light_actor.lock(&self.actor_ref);

        button_actor_locked && water_actor_locked && boiler_actor_locked && brew_actor_locked && temperature_sensor_actor_locked && power_light_actor_locked && status_light_actor_locked
    }

    fn release_locks(&self) {
        self.status_light_actor.unlock(&self.actor_ref);
        self.power_light_actor.unlock(&self.actor_ref);
        self.temperature_sensor_actor.unlock(&self.actor_ref);
        self.brew_actor.unlock(&self.actor_ref);
        self.boiler_actor.unlock(&self.actor_ref);
        self.water_actor.unlock(&self.actor_ref);
        self.button_actor.unlock(&self.actor_ref);
    }

    fn cleanup(&self) {
        self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(false));
        self.status_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(false));
        self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));

        self.release_locks();
    }

    async fn blink_update_indicator( actor_ref: ActorRef<UpdateMessage>, power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>) {
        loop {
            power_light_actor.tell(Some(&actor_ref), LightMessage::BlinkTimes(1));
            status_light_actor.tell(Some(&actor_ref), LightMessage::BlinkTimes(1));

            Timer::after(UPDATE_INDICATOR_INTERVAL).await;
        }
    }

    async fn start(&mut self) {
        if self.acquire_locks() {
            self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
            self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));

            Timer::after(SAFE_SHUTDOWN_DELAY).await;

            match self.update_port.confirm() {
                Ok(UpdateConfirmation::Confirmed) => {}
                Ok(UpdateConfirmation::NotPending) => {
                    let actor_ref = self.actor_ref.clone();
                    let power_light_actor = self.power_light_actor.clone();
                    let status_light_actor = self.status_light_actor.clone();

                    let result = {
                        let update = self.update_port.update();

                        select(update, Self::blink_update_indicator(actor_ref, power_light_actor, status_light_actor)).await
                    };

                    if let Either::First(Err(_)) = result {
                       self.status_light_actor.tell(Some(&self.actor_ref), LightMessage::BlinkTimes(4));
                    }
                }
                Err(_) => self.status_light_actor.tell(Some(&self.actor_ref), LightMessage::BlinkTimes(2)),
            }
        }

        self.cleanup();
    }
}

impl<U: UpdatePort> Actor for UpdateActor<U> {
    type Message = UpdateMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            UpdateMessage::Start => self.start().await,
        }
    }
}
