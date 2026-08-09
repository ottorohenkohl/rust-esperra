use crate::{
    domain::{
        clock::Instant,
        config::port::{HeaterOptions, PidGains},
        gpio::{output::OutputMessage, sensor::TemperatureSensorMessage},
    },
    library::actor::{Actor, ActorRef},
};

use super::pid::PidService;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerMessage {
    EvaluateHeating,
    RegulateHeating { measured_temperature: f64 },
    StopHeating,
}

pub struct BoilerActor<S, H, I> {
    heater_options: HeaterOptions,
    pid_service: PidService,
    temperature_sensor: S,
    heating_element: H,
    cycle_start: I,
}

impl<S, H, I> BoilerActor<S, H, I> {
    pub fn new(
        heater_options: HeaterOptions,
        pid_gains: PidGains,
        temperature_sensor: S,
        heating_element: H,
        cycle_start: I,
    ) -> Self {
        Self {
            heater_options,
            pid_service: PidService::new(pid_gains, heater_options.heating_cycle_ms),
            temperature_sensor,
            heating_element,
            cycle_start,
        }
    }

    fn should_heat(&mut self, temperature: f64) -> bool {
        let cycle_elapsed_ms = self.cycle_start.elapsed_ms();
        if cycle_elapsed_ms >= self.heater_options.heating_cycle_ms {
            self.cycle_start.restart();
            return false;
        }

        if temperature
            < self.heater_options.target_temperature - self.heater_options.boost_intervall_size
        {
            return true;
        }

        let pid_percentage = self
            .pid_service
            .compute(self.heater_options.target_temperature, temperature);
        let heating_window_ms =
            self.heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0;

        (cycle_elapsed_ms as f64) < heating_window_ms
    }
}

impl<S, H, I> Actor for BoilerActor<S, H, I>
where
    S: ActorRef<TemperatureSensorMessage>,
    H: ActorRef<OutputMessage>,
    I: Instant,
{
    type Message = BoilerMessage;

    fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::EvaluateHeating => {
                self.temperature_sensor
                    .send(TemperatureSensorMessage::MeasureTemperature);
            }
            BoilerMessage::RegulateHeating {
                measured_temperature,
            } => {
                let active = self.should_heat(measured_temperature);

                self.heating_element.send(OutputMessage::SetActive(active));
            }
            BoilerMessage::StopHeating => {
                self.heating_element.send(OutputMessage::SetActive(false));
            }
        }
    }
}
