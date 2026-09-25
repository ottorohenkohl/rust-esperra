use crate::domain::config::port::ReadConfigPort;

use alloc::sync::Arc;
use embassy_time::Instant;

pub struct PidService<C> {
    config_provider: Arc<C>,
    last_temperature: f64,
    last_update: Instant,
    stored_integral: f64,
}

impl<C> PidService<C> where C: ReadConfigPort {
    pub fn new(config_provider: Arc<C>) -> Self {
        let last_temperature = 0.0;
        let stored_integral = 0.0;

        let last_update = Instant::now();

        Self { config_provider, last_temperature, stored_integral, last_update }
    }

    pub fn compute(&mut self, actual_temperature: f64, target_temperature: f64) -> f64 {
        let now = Instant::now();

        let pid_gains = self.config_provider.read_pid_gains();
        let interval_size = self.current_interval_size(now);

        let temperature_error = target_temperature - actual_temperature;

        let proportional = pid_gains.k_proportional * temperature_error;
        let derivative = pid_gains.k_derivative * (self.last_temperature - actual_temperature) / interval_size;
        let raw_integral = pid_gains.k_intergral * temperature_error * interval_size + self.stored_integral;

        let raw_output = proportional + raw_integral + derivative;

        self.last_update = now;
        self.last_temperature = actual_temperature;

        let not_undershooting = raw_output <= 100.0 || temperature_error <= 0.0;
        let not_overshooting = raw_output >= 0.0 || temperature_error >= 0.0;

        if not_undershooting && not_overshooting {
            self.stored_integral = raw_integral;
        }

        (proportional + self.stored_integral + derivative).clamp(0.0, 100.0)
    }

    fn current_interval_size(&self, now: Instant) -> f64 {
        (now - self.last_update).as_millis() as f64 / 1000.0
    }
}
