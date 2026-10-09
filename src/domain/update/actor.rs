use super::port::{UpdateConfirmation, UpdatePort};
use super::protocol::UpdateMessage;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::controls::protocol::{ButtonMessage, WaterMessage};
use crate::domain::gpio::protocol::{LightMode, LightingMessage, TemperatureSensorMessage};
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use core::sync::atomic::{AtomicBool, Ordering};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};

const SAFE_SHUTDOWN_DELAY: Duration = Duration::from_secs(2);
const UPDATE_INDICATOR_INTERVAL: Duration = Duration::from_millis(500);

static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn update_in_progress() -> bool {
    UPDATE_IN_PROGRESS.load(Ordering::Acquire)
}

pub struct UpdateActor<U: UpdatePort> {
    public_name: &'static str,

    actor_ref: ActorRef<UpdateMessage>,
    update_port: U,
    button_actor: ActorRef<ButtonMessage>,
    water_actor: ActorRef<WaterMessage>,
    boiler_actor: ActorRef<BoilerMessage>,
    brew_actor: ActorRef<BrewMessage>,
    temperature_sensor_actor: ActorRef<TemperatureSensorMessage>,
    lighting_actor: ActorRef<LightingMessage>,
}

impl<U: UpdatePort> UpdateActor<U> {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<UpdateMessage>, update_port: U, button_actor: ActorRef<ButtonMessage>, water_actor: ActorRef<WaterMessage>, boiler_actor: ActorRef<BoilerMessage>, brew_actor: ActorRef<BrewMessage>, temperature_sensor_actor: ActorRef<TemperatureSensorMessage>, lighting_actor: ActorRef<LightingMessage>) -> Self {
        Self { public_name, actor_ref, update_port, button_actor, water_actor, boiler_actor, brew_actor, temperature_sensor_actor, lighting_actor }
    }

    fn acquire_locks(&self) -> bool {
        let button_actor_locked = self.button_actor.lock(&self.actor_ref);
        let water_actor_locked = self.water_actor.lock(&self.actor_ref);
        let boiler_actor_locked = self.boiler_actor.lock(&self.actor_ref);
        let brew_actor_locked = self.brew_actor.lock(&self.actor_ref);
        let temperature_sensor_actor_locked = self.temperature_sensor_actor.lock(&self.actor_ref);
        let lighting_actor_locked = self.lighting_actor.lock(&self.actor_ref);

        button_actor_locked && water_actor_locked && boiler_actor_locked && brew_actor_locked && temperature_sensor_actor_locked && lighting_actor_locked
    }

    fn release_locks(&self) {
        self.lighting_actor.unlock(&self.actor_ref);
        self.temperature_sensor_actor.unlock(&self.actor_ref);
        self.brew_actor.unlock(&self.actor_ref);
        self.boiler_actor.unlock(&self.actor_ref);
        self.water_actor.unlock(&self.actor_ref);
        self.button_actor.unlock(&self.actor_ref);
    }

    fn cleanup(&self, error_blinks: Option<u8>) {
        self.lighting_actor.tell(Some(&self.actor_ref), LightingMessage::SetBoth(LightMode::Off, LightMode::Off));

        if let Some(times) = error_blinks {
            self.lighting_actor.tell(Some(&self.actor_ref), LightingMessage::BlinkStatusTimes(times));
        }

        self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));

        self.release_locks();
    }

    async fn blink_update_indicator(actor_ref: ActorRef<UpdateMessage>, lighting_actor: ActorRef<LightingMessage>) {
        loop {
            lighting_actor.tell(Some(&actor_ref), LightingMessage::SetBoth(LightMode::Blinking, LightMode::Blinking));

            Timer::after(UPDATE_INDICATOR_INTERVAL).await;
        }
    }

    async fn start(&mut self) {
        UPDATE_IN_PROGRESS.store(true, Ordering::Release);

        let mut error_blinks = None;

        if self.acquire_locks() {
            self.boiler_actor.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
            self.brew_actor.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));

            Timer::after(SAFE_SHUTDOWN_DELAY).await;

            match self.update_port.confirm().await {
                Ok(UpdateConfirmation::Confirmed) => {}
                Ok(UpdateConfirmation::NotPending) => {
                    let actor_ref = self.actor_ref.clone();
                    let lighting_actor = self.lighting_actor.clone();

                    let result = select(self.update_port.update(), Self::blink_update_indicator(actor_ref, lighting_actor)).await;

                    if let Either::First(Err(_)) = result {
                        error_blinks = Some(4);
                    }
                }
                Err(_) => error_blinks = Some(2),
            }
        }

        self.cleanup(error_blinks);

        UPDATE_IN_PROGRESS.store(false, Ordering::Release);
    }
}

impl<U: UpdatePort> Actor for UpdateActor<U> {
    type Message = UpdateMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            UpdateMessage::Start => self.start().await,
        }
    }
}
