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
    actor_ref: ActorRef<BoilerMessage>,
    config: Arc<C>,
    current_temperature: f64,
    cycle_start: Instant,
    heating_output_port: H,
    pid_service: PidService<C>,
    power_light_actor: ActorRef<LightMessage>
}

impl<C, H> BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    pub fn new(actor_ref: ActorRef<BoilerMessage>, config: Arc<C>, heating_output_port: H, power_light_actor: ActorRef<LightMessage>) -> Self {
        let pid_service = PidService::new(Arc::clone(&config));
        let cycle_start = Instant::now();

        let current_temperature = 21.0;

        Self { actor_ref, config, current_temperature, cycle_start, heating_output_port, pid_service, power_light_actor }
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

    async fn switch_heater(&mut self) {
        self.update_cycle();

        let should_heat = self.should_heat();
        let heater_options = self.config.read_heater_options();

        self.heating_output_port.set_active(should_heat).await;

        let lower_bound = heater_options.target_temperature - heater_options.target_temperature_interval;
        let upper_bound = heater_options.target_temperature + heater_options.target_temperature_interval;

        match self.current_temperature {
            current_temperature if (lower_bound..upper_bound).contains(&current_temperature) => {
                self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::SetActive(true));
            }
            _ => {
                self.power_light_actor.tell(Some(&self.actor_ref), LightMessage::BlinkTimes(1));
            }
        }
    }
}

impl<C, H> Actor for BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    type Message = BoilerMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::SwitchHeater => {
                self.switch_heater().await;
            }
            BoilerMessage::UpdateTemperature(measured_temperature) => {
                self.current_temperature = measured_temperature;
            }
            BoilerMessage::SetActive(active) => {
                self.heating_output_port.set_active(active).await;
            }
        }
    }
}
