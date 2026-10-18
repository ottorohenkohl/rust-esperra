use super::protocol::CleaningMessage;
use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::protocol::ConfigMessage;
use crate::domain::controls::protocol::ButtonMessage;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_time::{Duration, Instant};

const CLEANING_TEMPERATURE: f64 = 125.0;
const CYCLE_INTERVAL: Duration = Duration::from_secs(10);
const PREPARATION_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const MAX_TEMPERATURE_AGE: Duration = Duration::from_secs(5);

static CLEANING_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static ENOUGH_WATER: AtomicBool = AtomicBool::new(false);

pub fn cleaning_in_progress() -> bool {
    CLEANING_IN_PROGRESS.load(Ordering::Acquire)
}

pub fn set_enough_water(enough: bool) {
    ENOUGH_WATER.store(enough, Ordering::Release);
}

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    WaitingForWater(Instant),
    Preparing(Instant),
    Cycling { deadline: Instant, pump_active: bool, completed: u8 },
}

pub struct CleaningActor {
    actor_ref: ActorRef<CleaningMessage>,
    boiler: ActorRef<BoilerMessage>,
    brew: ActorRef<BrewMessage>,
    button: ActorRef<ButtonMessage>,
    energy: ActorRef<EnergyMessage>,
    config: ActorRef<ConfigMessage>,
    update: ActorRef<UpdateMessage>,
    temperature: Option<(f64, Instant)>,
    phase: Phase,
}

impl CleaningActor {
    pub fn new(actor_ref: ActorRef<CleaningMessage>, boiler: ActorRef<BoilerMessage>, brew: ActorRef<BrewMessage>, button: ActorRef<ButtonMessage>, energy: ActorRef<EnergyMessage>, config: ActorRef<ConfigMessage>, update: ActorRef<UpdateMessage>) -> Self {
        Self { actor_ref, boiler, brew, button, energy, config, update, temperature: None, phase: Phase::Idle }
    }

    fn release_locks(&self) {
        self.update.unlock(&self.actor_ref);
        self.config.unlock(&self.actor_ref);
        self.energy.unlock(&self.actor_ref);
        self.button.unlock(&self.actor_ref);
        self.brew.unlock(&self.actor_ref);
        self.boiler.unlock(&self.actor_ref);
        self.actor_ref.unlock(&self.actor_ref);
    }

    fn start(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            self.phase = Phase::WaitingForWater(Instant::now());
        }
    }

    fn acquire_locks(&mut self) {
        let locked = self.actor_ref.lock(&self.actor_ref)
            && self.boiler.lock(&self.actor_ref)
            && self.brew.lock(&self.actor_ref)
            && self.button.lock(&self.actor_ref)
            && self.energy.lock(&self.actor_ref)
            && self.config.lock(&self.actor_ref)
            && self.update.lock(&self.actor_ref);

        if !locked {
            self.release_locks();
            self.phase = Phase::Idle;
            return;
        }

        CLEANING_IN_PROGRESS.store(true, Ordering::Release);
        self.brew.tell(Some(&self.actor_ref), BrewMessage::SetCleaningPump(false));
        self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetCleaningTemperature(Some(CLEANING_TEMPERATURE)));
        self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetActive(false));
        self.phase = Phase::Preparing(Instant::now());
    }

    fn finish(&mut self) {
        self.brew.tell(Some(&self.actor_ref), BrewMessage::SetCleaningPump(false));
        // Removing the override restores the unchanged configured setpoint.
        self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetCleaningTemperature(None));
        self.phase = Phase::Idle;
        self.release_locks();
        CLEANING_IN_PROGRESS.store(false, Ordering::Release);
    }

    fn tick(&mut self) {
        if matches!(self.phase, Phase::Idle) {
            return;
        }

        let enough_water = ENOUGH_WATER.load(Ordering::Acquire);
        let valid_temperature = self.temperature.filter(|(value, measured)| value.is_finite() && (2.0..=125.0).contains(value) && measured.elapsed() <= MAX_TEMPERATURE_AGE);
        if !matches!(self.phase, Phase::WaitingForWater(_)) {
            self.boiler.tell(Some(&self.actor_ref), BoilerMessage::SetActive(enough_water));
        }

        match self.phase {
            Phase::WaitingForWater(started) => {
                if started.elapsed() >= PREPARATION_TIMEOUT {
                    self.phase = Phase::Idle;
                } else if enough_water {
                    self.acquire_locks();
                }
            }
            Phase::Preparing(started) => {
                if started.elapsed() >= PREPARATION_TIMEOUT {
                    self.finish();
                } else if enough_water && valid_temperature.is_some_and(|(value, _)| value >= CLEANING_TEMPERATURE) {
                    self.brew.tell(Some(&self.actor_ref), BrewMessage::SetCleaningPump(true));
                    self.phase = Phase::Cycling { deadline: Instant::now() + CYCLE_INTERVAL, pump_active: true, completed: 0 };
                }
            }
            Phase::Cycling { deadline, pump_active, completed } => {
                if !enough_water || valid_temperature.is_none() {
                    self.finish();
                } else if Instant::now() >= deadline {
                    if !pump_active && completed == 5 {
                        self.finish();
                        return;
                    }
                    self.brew.tell(Some(&self.actor_ref), BrewMessage::SetCleaningPump(!pump_active));
                    self.phase = Phase::Cycling { deadline: Instant::now() + CYCLE_INTERVAL, pump_active: !pump_active, completed: completed + u8::from(pump_active) };
                }
            }
            Phase::Idle => {}
        }
    }
}

impl Actor for CleaningActor {
    type Message = CleaningMessage;

    fn name(&self) -> &'static str {
        "cleaning"
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            CleaningMessage::Start => self.start(),
            CleaningMessage::Tick => self.tick(),
            CleaningMessage::UpdateTemperature(reading) => self.temperature = reading.map(|value| (value, Instant::now())),
        }
    }
}
