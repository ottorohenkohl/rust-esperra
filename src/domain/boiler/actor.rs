use crate::domain::{
    actor::ActorRef,
    config::port::{HeaterOptions, PidGains},
    output::OutputMessage,
    sensor::TemperatureSensorMessage,
};

use super::pid::PidService;

pub enum BoilerMessage {
    Tick { cycle_elapsed_ms: u64 },
    TemperatureMeasured(Result<f64, TemperatureReadError>),
}

pub enum TemperatureReadError {
    ReadFailed,
}

pub struct BoilerActor<S, H> {
    heater_options: HeaterOptions,
    pid_service: PidService,
    temperature_sensor: S,
    heating_element: H,
    cycle_elapsed_ms: u64,
}

impl<S, H> BoilerActor<S, H>
where
    S: ActorRef<TemperatureSensorMessage>,
    H: ActorRef<OutputMessage>,
{
    pub fn new(
        heater_options: HeaterOptions,
        pid_gains: PidGains,
        temperature_sensor: S,
        heating_element: H,
    ) -> Self {
        Self {
            heater_options,
            pid_service: PidService::new(pid_gains, heater_options.heating_cycle_ms),
            temperature_sensor,
            heating_element,
            cycle_elapsed_ms: 0,
        }
    }

    pub fn handle(&mut self, message: BoilerMessage) {
        match message {
            BoilerMessage::Tick { cycle_elapsed_ms } => {
                self.cycle_elapsed_ms = cycle_elapsed_ms;
                let _ = self
                    .temperature_sensor
                    .try_send(TemperatureSensorMessage::Measure);
            }
            BoilerMessage::TemperatureMeasured(Ok(temperature)) => {
                let active = self.should_heat(temperature);
                let _ = self
                    .heating_element
                    .try_send(OutputMessage::SetActive(active));
            }
            BoilerMessage::TemperatureMeasured(Err(_)) => {
                let _ = self
                    .heating_element
                    .try_send(OutputMessage::SetActive(false));
            }
        }
    }

    fn should_heat(&mut self, temperature: f64) -> bool {
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

        (self.cycle_elapsed_ms as f64) < heating_window_ms
    }
}
