use super::protocol::BoilerMessage;
use super::service::PidService;

use crate::domain::config::port::ConfigPort;
use crate::domain::gpio::protocol::OutputMessage;
use crate::library::actor::{Actor, ActorRef};

use alloc::sync::Arc;
use embassy_time::Instant;

pub struct BoilerActor<C> {
    config: Arc<C>,
    pid_service: PidService<C>,
    heating_element: ActorRef<OutputMessage>,
    cycle_start: Instant,
}

impl<C> BoilerActor<C>
where
    C: ConfigPort,
{
    pub fn new(config: Arc<C>, heating_element: ActorRef<OutputMessage>) -> Self {
        let pid_service = PidService::new(Arc::clone(&config));

        Self { config, pid_service, heating_element, cycle_start: Instant::now() }
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

impl<C> Actor for BoilerActor<C>
where
    C: ConfigPort,
{
    type Message = BoilerMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::RegulateHeating { measured_temperature } => {
                let message = OutputMessage::SetActive(self.should_heat(measured_temperature));

                self.heating_element.tell(message);
            }
            BoilerMessage::StopHeating => {
                let message = OutputMessage::SetActive(false);

                self.heating_element.tell(message);
            }
        }
    }
}
