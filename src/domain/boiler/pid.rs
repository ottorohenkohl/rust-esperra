use crate::domain::config::port::ConfigProvider;

use alloc::sync::Arc;

pub struct Pid {
    config_provider: Arc<dyn ConfigProvider>,

    last_temperature: f64,
    stored_integral: f64,
}

impl Pid {
    pub fn new(config_provider: Arc<dyn ConfigProvider>) -> Self {
        let last_temperature = 0.0;
        let stored_integral = 0.0;

        Self { config_provider, last_temperature, stored_integral }
    }

    pub fn compute(&mut self, target_temperature: f64, actual_temperature: f64) -> f64 {
        let pid_gains = self.config_provider.read_pid_gains();
        let timing_intervals = self.config_provider.read_heater_options();

        let interval_size = (timing_intervals.heating_cycle_ms as f64 / 1000.0).max(f64::EPSILON);

        let temperature_error = target_temperature - actual_temperature;

        let temperature_change = self.last_temperature - actual_temperature;

        let proportional = pid_gains.k_proportional * temperature_error;
        let derivative = pid_gains.k_derivative * temperature_change / interval_size;
        let raw_integral = pid_gains.k_intergral * temperature_error * interval_size + self.stored_integral;

        let raw_output = proportional + raw_integral + derivative;

        self.last_temperature = actual_temperature;

        let not_undershooting = raw_output <= 100.0 || temperature_error <= 0.0;
        let not_overshooting = raw_output >= 0.0 || temperature_error >= 0.0;

        if not_undershooting && not_overshooting {
            self.stored_integral = raw_integral
        }

        (proportional + self.stored_integral + derivative).clamp(0.0, 100.0)
    }
}
