use crate::domain::config::port::{HeaterOptions, PidGains};

use super::pid::PidService;

pub enum BoilerEvent {
    Tick { cycle_elapsed_ms: u64 },
    TemperatureRead(Result<f64, TemperatureReadError>),
}

pub enum TemperatureReadError {
    ReadFailed,
}

pub enum BoilerEffect {
    ReadTemperature,
    SetHeating(bool),
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

    pub fn handle(&mut self, event: BoilerEvent) -> BoilerEffect {
        match event {
            BoilerEvent::Tick { cycle_elapsed_ms } => {
                self.cycle_elapsed_ms = cycle_elapsed_ms;
                BoilerEffect::ReadTemperature
            }
            BoilerEvent::TemperatureRead(Ok(temperature)) => {
                BoilerEffect::SetHeating(self.should_heat(temperature))
            }
            BoilerEvent::TemperatureRead(Err(_)) => BoilerEffect::SetHeating(false),
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
