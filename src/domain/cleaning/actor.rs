use super::protocol::CleaningMessage;
use crate::domain::boiler::protocol::BoilerMessage;
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
const PREPARATION_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const MAX_TEMPERATURE_AGE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    WaitingForLocks(Instant),
    Preparing(Instant),
    Cycling { deadline: Instant, pump_active: bool, completed: u8 },
}

pub struct CleaningActor<C> {
    config_port: Arc<C>,
    previous_temperature_options: Option<TemperatureOptions>,
    actor_ref: ActorRef<CleaningMessage>,
    boiler: ActorRef<BoilerMessage>,
    brew: ActorRef<BrewMessage>,
    button: ActorRef<ButtonMessage>,
    water: ActorRef<WaterMessage>,
    energy: ActorRef<EnergyMessage>,
    config: ActorRef<ConfigMessage>,
    update: ActorRef<UpdateMessage>,
    temperature: Option<(f64, Instant)>,
    phase: Phase,
}

impl<C: ReadConfigPort> CleaningActor<C> {
    pub fn new(config_port: Arc<C>, actor_ref: ActorRef<CleaningMessage>, boiler: ActorRef<BoilerMessage>, brew: ActorRef<BrewMessage>, button: ActorRef<ButtonMessage>, water: ActorRef<WaterMessage>, energy: ActorRef<EnergyMessage>, config: ActorRef<ConfigMessage>, update: ActorRef<UpdateMessage>) -> Self {
        Self { config_port, previous_temperature_options: None, actor_ref, boiler, brew, button, water, energy, config, update, temperature: None, phase: Phase::Idle }
    }

    fn release_locks(&self) {
        self.update.unlock(&self.actor_ref);
        self.config.unlock(&self.actor_ref);
        self.energy.unlock(&self.actor_ref);
        self.water.unlock(&self.actor_ref);
        self.button.unlock(&self.actor_ref);
        self.brew.unlock(&self.actor_ref);
        self.boiler.unlock(&self.actor_ref);
        self.actor_ref.unlock(&self.actor_ref);
    }

    fn start(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            self.phase = Phase::WaitingForLocks(Instant::now());
        }
    }

    fn acquire_locks(&mut self) {
        let locked = self.actor_ref.lock(&self.actor_ref)
            && self.boiler.lock(&self.actor_ref)
            && self.brew.lock(&self.actor_ref)
            && self.water.lock(&self.actor_ref)
            && self.button.lock(&self.actor_ref)
            && self.energy.lock(&self.actor_ref)
            && self.config.lock(&self.actor_ref)
            && self.update.lock(&self.actor_ref);

        if !locked {
            self.release_locks();
            return;
        }

        self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
        self.previous_temperature_options = Some(self.config_port.read_temperature_options());
        self.config.tell(Some(&self.actor_ref), ConfigMessage::UpdateTemperatureOptions(TemperatureOptions { target_temperature: CLEANING_TEMPERATURE }));
        self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
        self.phase = Phase::Preparing(Instant::now());
    }

    fn finish(&mut self) {
        self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(false));
        if let Some(options) = self.previous_temperature_options.take() {
            self.config.tell(Some(&self.actor_ref), ConfigMessage::UpdateTemperatureOptions(options));
        }
        self.phase = Phase::Idle;
        self.release_locks();
    }

    fn tick(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            return;
        }

        let valid_temperature = self.temperature.filter(|(value, measured)| value.is_finite() && *value >= 2.0 && measured.elapsed() <= MAX_TEMPERATURE_AGE);
        if !matches!(self.phase, Phase::WaitingForLocks(_)) {
            self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetActive(true));
        }

        match self.phase {
            Phase::WaitingForLocks(started) => {
                if started.elapsed() >= PREPARATION_TIMEOUT {
                    self.phase = Phase::Idle;
                } else {
                    self.acquire_locks();
                }
            }
            Phase::Preparing(started) => {
                if started.elapsed() >= PREPARATION_TIMEOUT {
                    self.finish();
                } else if valid_temperature.is_some_and(|(value, _)| value >= CLEANING_TEMPERATURE) {
                    self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(true));
                    self.phase = Phase::Cycling { deadline: Instant::now() + CYCLE_INTERVAL, pump_active: true, completed: 0 };
                }
            }
            Phase::Cycling { deadline, pump_active, completed } => {
                self.brew.tell(Some(&self.actor_ref), BrewMessage::Tick);
                if valid_temperature.is_none() {
                    self.finish();
                } else if Instant::now() >= deadline {
                    if !pump_active && completed == 5 {
                        self.finish();
                        return;
                    }
                    self.brew.tell(Some(&self.actor_ref), BrewMessage::SetBrewing(!pump_active));
                    self.phase = Phase::Cycling { deadline: Instant::now() + CYCLE_INTERVAL, pump_active: !pump_active, completed: completed + u8::from(pump_active) };
                }
            }
            Phase::Idle => {}
        }
    }
}

impl<C: ReadConfigPort> Actor for CleaningActor<C> {
    type Message = CleaningMessage;

    fn name(&self) -> &'static str {
        "cleaning"
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CleaningMessage::Start => self.start(),
            CleaningMessage::Tick => self.tick(),
            CleaningMessage::UpdateTemperature(reading) => {
                self.temperature = reading.map(|value| (value, Instant::now()));
                // Forward sensor readings as lock owner while the boiler is reserved.
                if matches!(self.phase, Phase::Preparing(_) | Phase::Cycling { .. }) {
                    let message = match reading {
                        Some(value) => BoilerMessage::UpdateTemperature(value),
                        None => BoilerMessage::InvalidateTemperature,
                    };
                    self.boiler.tell(Some(&self.actor_ref), message);
                }
            }
        }
    }
}
