use crate::{
    actor::Actor,
    config::port::{HeaterOptions, PidGains},
    output::{OutputCommand, OutputState},
    sensor::{TemperatureSensorCommand, TemperatureSensorEvent},
};

use super::pid::PidService;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerCommand {
    Tick { cycle_elapsed_ms: u64 },
    Temperature(TemperatureSensorEvent),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoilerEffect {
    MeasureTemperature(TemperatureSensorCommand),
    ControlHeatingElement(OutputCommand),
}

pub struct BoilerActor {
    heater_options: HeaterOptions,
    pid_service: PidService,
    cycle_elapsed_ms: u64,
}

impl BoilerActor {
    pub fn new(heater_options: HeaterOptions, pid_gains: PidGains) -> Self {
        Self {
            heater_options,
            pid_service: PidService::new(pid_gains, heater_options.heating_cycle_ms),
            cycle_elapsed_ms: 0,
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

impl Actor for BoilerActor {
    type Message = BoilerCommand;
    type Effect = BoilerEffect;

    fn handle(&mut self, message: Self::Message) -> Self::Effect {
        match message {
            BoilerCommand::Tick { cycle_elapsed_ms } => {
                self.cycle_elapsed_ms = cycle_elapsed_ms;
                BoilerEffect::MeasureTemperature(TemperatureSensorCommand::Measure)
            }
            BoilerCommand::Temperature(TemperatureSensorEvent::Measured(temperature)) => {
                let state = if self.should_heat(temperature) {
                    OutputState::Active
                } else {
                    OutputState::Inactive
                };

                BoilerEffect::ControlHeatingElement(OutputCommand::Set(state))
            }
            BoilerCommand::Temperature(TemperatureSensorEvent::MeasurementFailed) => {
                BoilerEffect::ControlHeatingElement(OutputCommand::Set(OutputState::Inactive))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> BoilerActor {
        BoilerActor::new(
            HeaterOptions {
                heating_cycle_ms: 100_000,
                target_temperature: 65.0,
                boost_intervall_size: 5.0,
            },
            PidGains {
                k_proportional: 1.0,
                k_intergral: 0.0,
                k_derivative: 0.0,
            },
        )
    }

    #[test]
    fn tick_requests_a_temperature_measurement() {
        assert_eq!(
            actor().handle(BoilerCommand::Tick {
                cycle_elapsed_ms: 10,
            }),
            BoilerEffect::MeasureTemperature(TemperatureSensorCommand::Measure)
        );
    }

    #[test]
    fn sensor_failure_switches_heater_off() {
        assert_eq!(
            actor().handle(BoilerCommand::Temperature(
                TemperatureSensorEvent::MeasurementFailed,
            )),
            BoilerEffect::ControlHeatingElement(OutputCommand::Set(OutputState::Inactive))
        );
    }

    #[test]
    fn low_temperature_switches_heater_on() {
        assert_eq!(
            actor().handle(BoilerCommand::Temperature(
                TemperatureSensorEvent::Measured(50.0),
            )),
            BoilerEffect::ControlHeatingElement(OutputCommand::Set(OutputState::Active))
        );
    }
}
