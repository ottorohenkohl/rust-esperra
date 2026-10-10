use super::protocol::BoilerMessage;
use super::service::PidService;

use crate::domain::config::port::ReadConfigPort;
use crate::domain::gpio::port::OutputPort;
use crate::domain::gpio::protocol::{LightMode, LightingMessage};
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;
use embassy_time::{Duration, Instant};

const MAX_TEMPERATURE_AGE: Duration = Duration::from_secs(5);

const MIN_VALID_TEMPERATURE: f64 = 2.0;
const MAX_VALID_TEMPERATURE: f64 = 125.0;

pub struct BoilerActor<C, H> {
    public_name: &'static str,

    actor_ref: ActorRef<BoilerMessage>,
    config_port: Arc<C>,
    current_temperature: Option<f64>,
    last_temperature_update: Option<Instant>,
    active: bool,
    brewing: bool,
    cycle_start: Instant,
    heating_output_port: H,
    pid_service: PidService<C>,
    lighting_actor: ActorRef<LightingMessage>
}

impl<C, H> BoilerActor<C, H> where C: ReadConfigPort, H: OutputPort {
    pub fn new(public_name: &'static str, actor_ref: ActorRef<BoilerMessage>, config_port: Arc<C>, heating_output_port: H, lighting_actor: ActorRef<LightingMessage>) -> Self {
        let pid_service = PidService::new(Arc::clone(&config_port));
        let cycle_start = Instant::now();

        let current_temperature = None;
        let last_temperature_update = None;

        let active = true;
        let brewing = false;

        Self { public_name, actor_ref, config_port, current_temperature, last_temperature_update, active, brewing, cycle_start, heating_output_port, pid_service, lighting_actor }
    }

    fn update_cycle(&mut self) {
        let heater_options = self.config_port.read_heater_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed().as_millis();

        if cycle_elapsed_ms >= heater_options.heating_cycle_ms {
            self.cycle_start = Instant::now();
        }
    }

    fn has_valid_temperature(&self) -> bool {
        let Some(current_temperature) = self.current_temperature else { return false };
        let Some(last_temperature_update) = self.last_temperature_update else { return false };

        let in_valid_temperature_range = (MIN_VALID_TEMPERATURE..MAX_VALID_TEMPERATURE).contains(&current_temperature);
        let in_valid_time_range = last_temperature_update.elapsed() <= MAX_TEMPERATURE_AGE;

        current_temperature.is_finite() && in_valid_temperature_range && in_valid_time_range
    }

    fn should_heat(&mut self) -> bool {
        if !self.active || !self.has_valid_temperature() {
            self.pid_service.reset();

            return false;
        }

        let current_temperature = self.current_temperature.unwrap();
        let heater_options = self.config_port.read_heater_options();
        let temperature_options = self.config_port.read_temperature_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed().as_millis();

        if self.brewing || current_temperature < temperature_options.target_temperature - heater_options.boost_interval_size {
            self.pid_service.reset();

            return true;
        }

        let pid_percentage = self.pid_service.compute(current_temperature, temperature_options.target_temperature);

        (cycle_elapsed_ms as f64) < heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0
    }

    fn invalidate_temperature(&mut self) {
        self.current_temperature = None;
        self.last_temperature_update = None;
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn set_brewing(&mut self, active: bool) {
        self.brewing = active;
        self.pid_service.reset();
    }

    fn update_temperature(&mut self, measured_temperature: f64) {
        self.current_temperature = Some(measured_temperature);
        self.last_temperature_update = Some(Instant::now());
    }

    async fn switch_heater(&mut self) {
        self.update_cycle();

        let should_heat = self.should_heat();
        let heater_options = self.config_port.read_heater_options();
        let temperature_options = self.config_port.read_temperature_options();

        self.heating_output_port.set_active(should_heat).await;

        let lower_bound = temperature_options.target_temperature - heater_options.target_temperature_interval;
        let upper_bound = temperature_options.target_temperature + heater_options.target_temperature_interval;

        match self.current_temperature {
            Some(current_temperature) if (lower_bound..upper_bound).contains(&current_temperature) => {
                self.lighting_actor.tell(Some(&self.actor_ref), LightingMessage::SetStatus(LightMode::On));
            }
            _ => {
                self.lighting_actor.tell(Some(&self.actor_ref), LightingMessage::SetStatus(LightMode::Blinking));
            }
        }
    }
}

impl<C, H> Actor for BoilerActor<C, H> where C: ReadConfigPort, H: OutputPort {
    type Message = BoilerMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::InvalidateTemperature => self.invalidate_temperature(),
            BoilerMessage::SetActive(active) => self.set_active(active),
            BoilerMessage::SetBrewing(active) => self.set_brewing(active),
            BoilerMessage::SwitchHeater => self.switch_heater().await,
            BoilerMessage::UpdateTemperature(measured_temperature) => self.update_temperature(measured_temperature),
        }
    }
}
