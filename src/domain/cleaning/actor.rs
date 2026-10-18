use super::protocol::CleaningMessage;

use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::{ReadConfigPort, TemperatureOptions};
use crate::domain::config::protocol::ConfigMessage;
use crate::domain::controls::protocol::{ButtonMessage, WaterMessage};
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;
use embassy_time::{Duration, Instant};

const CLEANING_TEMPERATURE: f64 = 125.0;
const CYCLE_INTERVAL: Duration = Duration::from_secs(10);

pub struct CleaningActor<C> {
    public_name: &'static str,

    config_port: Arc<C>,
    actor_ref: ActorRef<CleaningMessage>,
    brew: ActorRef<BrewMessage>,
    button: ActorRef<ButtonMessage>,
    water: ActorRef<WaterMessage>,
    energy: ActorRef<EnergyMessage>,
    config: ActorRef<ConfigMessage>,
    update: ActorRef<UpdateMessage>,
    previous_temperature_options: Option<TemperatureOptions>,
    next_step: Option<Instant>,
    step: u8,
}

impl<C: ReadConfigPort> CleaningActor<C> {
    pub fn new(public_name: &'static str, config_port: Arc<C>, actor_ref: ActorRef<CleaningMessage>, brew: ActorRef<BrewMessage>, button: ActorRef<ButtonMessage>, water: ActorRef<WaterMessage>, energy: ActorRef<EnergyMessage>, config: ActorRef<ConfigMessage>, update: ActorRef<UpdateMessage>) -> Self {
        let previous_temperature_options = None;
        let next_step = None;

        let step = 0;

        Self { public_name, config_port, actor_ref, brew, button, water, energy, config, update, previous_temperature_options, next_step, step }
    }

    fn release_locks(&self) {
        self.update.unlock(&self.actor_ref);
        self.config.unlock(&self.actor_ref);
        self.energy.unlock(&self.actor_ref);
        self.water.unlock(&self.actor_ref);
        self.button.unlock(&self.actor_ref);
        self.brew.unlock(&self.actor_ref);
    }

    fn start(&mut self) {
        if self.previous_temperature_options.is_some() {
            return;
        }

        let locked = self.brew.lock(&self.actor_ref)
            && self.water.lock(&self.actor_ref)
            && self.button.lock(&self.actor_ref)
            && self.energy.lock(&self.actor_ref)
            && self.config.lock(&self.actor_ref)
            && self.update.lock(&self.actor_ref);

        if locked {
            self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
            self.previous_temperature_options = Some(self.config_port.read_temperature_options());
            self.config.tell(Some(&self.actor_ref), ConfigMessage::UpdateTemperatureOptions(TemperatureOptions { target_temperature: CLEANING_TEMPERATURE }));

            return;
        }

        self.release_locks();
    }

    fn update_temperature(&mut self, reading: Option<f64>) {
        if self.previous_temperature_options.is_some() && self.next_step.is_none() && reading.is_some_and(|value| value >= CLEANING_TEMPERATURE) {
            self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(true));
            self.step = 1;
            self.next_step = Some(Instant::now() + CYCLE_INTERVAL);
        }
    }

    fn tick(&mut self) {
        let Some(deadline) = self.next_step else { return };

        self.brew.tell(Some(&self.actor_ref), BrewMessage::Tick);

        if Instant::now() < deadline {
            return;
        }

        if self.step == 10 {
            if let Some(options) = self.previous_temperature_options.take() {
                self.config.tell(Some(&self.actor_ref), ConfigMessage::UpdateTemperatureOptions(options));
            }

            self.next_step = None;

            self.release_locks();

            return;
        }

        self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(self.step % 2 == 0));
        self.step += 1;
        self.next_step = Some(Instant::now() + CYCLE_INTERVAL);
    }
}

impl<C: ReadConfigPort> Actor for CleaningActor<C> {
    type Message = CleaningMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CleaningMessage::Start => self.start(),
            CleaningMessage::Tick => self.tick(),
            CleaningMessage::UpdateTemperature(reading) => self.update_temperature(reading),
        }
    }
}
