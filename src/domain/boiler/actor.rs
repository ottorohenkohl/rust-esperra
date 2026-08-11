use super::protocol::BoilerMessage;
use super::service::PidService;

use crate::domain::config::port::ConfigPort;
use crate::domain::gpio::port::OutputPort;
use crate::library::actor::Actor;

use alloc::sync::Arc;
use embassy_time::Instant;

pub struct BoilerActor<C, H> {
    config: Arc<C>,
    cycle_start: Instant,
    heating_output_port: H,
    pid_service: PidService<C>,
}

impl<C, H> BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    pub fn new(config: Arc<C>, heating_output_port: H) -> Self {
        let pid_service = PidService::new(Arc::clone(&config));
        let cycle_start = Instant::now();

        Self { config, pid_service, heating_output_port, cycle_start }
    }

    fn should_heat(&mut self, temperature: f64) -> bool {
        let heater_options = self.config.read_heater_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed().as_millis();

        if cycle_elapsed_ms >= heater_options.heating_cycle_ms {
            self.cycle_start = Instant::now();
        }

        if temperature < heater_options.target_temperature - heater_options.boost_intervall_size {
            return true;
        }

        let pid_percentage = self.pid_service.compute(heater_options.target_temperature, temperature);

        (cycle_elapsed_ms as f64) < heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0
    }
}

impl<C, H> Actor for BoilerActor<C, H> where C: ConfigPort, H: OutputPort {
    type Message = BoilerMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::RegulateHeating { measured_temperature } => {
                let should_heat = self.should_heat(measured_temperature);

                self.heating_output_port.set_active(should_heat).await;
            }
            BoilerMessage::StopHeating => {
                self.heating_output_port.set_active(false).await;
            }
        }
    }
}
