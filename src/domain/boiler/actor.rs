use crate::domain::clock::Instant;
use crate::domain::config::port::ConfigPort;
use crate::domain::gpio::output::OutputMessage;
use crate::domain::gpio::sensor::TemperatureSensorMessage;
use crate::library::actor::Actor;
use crate::library::actor::ActorRef;
use alloc::sync::Arc;

use super::pid::PidService;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerMessage {
    EvaluateHeating,
    RegulateHeating { measured_temperature: f64 },
    StopHeating,
}

pub struct BoilerActor<C, I> {
    config: Arc<C>,
    pid_service: PidService<C>,
    temperature_sensor: ActorRef<TemperatureSensorMessage>,
    heating_element: ActorRef<OutputMessage>,
    cycle_start: I,
}

impl<C, I> BoilerActor<C, I>
where
    C: ConfigPort,
    I: Instant,
{
    pub fn new(config: Arc<C>, temperature_sensor: ActorRef<TemperatureSensorMessage>, heating_element: ActorRef<OutputMessage>, cycle_start: I) -> Self {
        let pid_service = PidService::new(Arc::clone(&config));

        Self { config, pid_service, temperature_sensor, heating_element, cycle_start }
    }

    fn should_heat(&mut self, temperature: f64) -> bool {
        let heater_options = *self.config.read_heater_options();
        let cycle_elapsed_ms = self.cycle_start.elapsed_ms();
        if cycle_elapsed_ms >= heater_options.heating_cycle_ms {
            self.cycle_start.restart();
            return false;
        }

        if temperature < heater_options.target_temperature - heater_options.boost_intervall_size {
            return true;
        }

        let pid_percentage = self.pid_service.compute(heater_options.target_temperature, temperature);
        let heating_window_ms = heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0;

        (cycle_elapsed_ms as f64) < heating_window_ms
    }
}

impl<C, I> Actor for BoilerActor<C, I>
where
    C: ConfigPort,
    I: Instant,
{
    type Message = BoilerMessage;

    fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::EvaluateHeating => {
                self.temperature_sensor.tell(TemperatureSensorMessage::MeasureTemperature);
            }
            BoilerMessage::RegulateHeating { measured_temperature } => {
                let active = self.should_heat(measured_temperature);

                self.heating_element.tell(OutputMessage::SetActive(active));
            }
            BoilerMessage::StopHeating => {
                self.heating_element.tell(OutputMessage::SetActive(false));
            }
        }
    }
}
