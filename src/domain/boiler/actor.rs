use super::protocol::BoilerMessage;
use super::service::PidService;

use crate::domain::config::port::ConfigPort;
use crate::domain::gpio::port::OutputPort;
use crate::domain::gpio::protocol::LightMessage;
use crate::library::actor::Actor;
use crate::library::actor::reference::ActorRef;

use alloc::sync::Arc;
use embassy_time::Instant;

pub struct BoilerActor<C, H> {
    config: Arc<C>,
    current_temperature: f64,
    cycle_start: Instant,
    heating_output_port: H,
    is_active: bool,
    pid_service: PidService<C>,
    power_light_actor: ActorRef<LightMessage>,
    status_light_actor: ActorRef<LightMessage>,
}

impl<C, H> BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    pub fn new(config: Arc<C>, heating_output_port: H, power_light_actor: ActorRef<LightMessage>, status_light_actor: ActorRef<LightMessage>) -> Self {
        let pid_service = PidService::new(Arc::clone(&config));
        let cycle_start = Instant::now();

        let current_temperature = 21.0;
        let is_active = true;

        Self { config, current_temperature, cycle_start, heating_output_port, is_active, pid_service, power_light_actor, status_light_actor }
    }

    fn update_cycle(&mut self) {
        let heater_options = self.config.read_heater_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed().as_millis();

        if cycle_elapsed_ms >= heater_options.heating_cycle_ms {
            self.cycle_start = Instant::now();
        }
    }

    fn should_heat(&mut self) -> bool {
        let heater_options = self.config.read_heater_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed().as_millis();

        if self.current_temperature < heater_options.target_temperature - heater_options.boost_intervall_size {
            return true;
        }

        let pid_percentage = self.pid_service.compute(self.current_temperature, heater_options.target_temperature);

        (cycle_elapsed_ms as f64) < heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0
    }
}

impl<C, H> Actor for BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    type Message = BoilerMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::SwitchHeater => {
                self.update_cycle();

                let shoult_heat = self.should_heat();

                self.heating_output_port.set_active(self.is_active && shoult_heat).await;
            }
            BoilerMessage::SafetyShutdown => {
                self.status_light_actor.tell(LightMessage::BlinkTimes(3));
            }
            BoilerMessage::UpdateTemperature { measured_temperature } => {
                self.current_temperature = measured_temperature;
            }
            BoilerMessage::SetActive { active } => {
                self.is_active = active;
            }
        }
    }
}
