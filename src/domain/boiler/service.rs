use crate::domain::config::port::ReadConfigPort;

use alloc::sync::Arc;
use core::option::Option;
use core::option::Option::{None, Some};
use embassy_time::Instant;

pub struct PidService<C> {
    config_provider: Arc<C>,
    last_temperature: f64,
    last_update: Option<Instant>,
    stored_integral: f64,
}

impl<C> PidService<C> where C: ReadConfigPort {
    pub fn new(config_provider: Arc<C>) -> Self {
        let last_temperature = 0.0;
        let stored_integral = 0.0;

        let last_update = None;

        Self { config_provider, last_temperature, stored_integral, last_update }
    }

    pub fn reset(&mut self) {
        self.last_update = None;

        self.last_temperature = 0.0;
        self.stored_integral = 0.0;
    }

    pub fn compute(&mut self, actual_temperature: f64, target_temperature: f64) -> f64 {
        let now = Instant::now();

        let pid_gains = self.config_provider.read_pid_gains();

        let temperature_error = target_temperature - actual_temperature;

        let proportional = pid_gains.k_proportional * temperature_error;

        let mut derivative = 0.0;
        let mut raw_integral = self.stored_integral;

        if let Some(interval_size) = self.current_interval_size(now) {
            derivative = pid_gains.k_derivative * (self.last_temperature - actual_temperature) / interval_size;
            raw_integral += pid_gains.k_integral * temperature_error * interval_size;
        }

        let raw_output = proportional + raw_integral + derivative;

        self.last_update = Some(now);
        self.last_temperature = actual_temperature;

        let not_undershooting = raw_output <= 100.0 || temperature_error <= 0.0;
        let not_overshooting = raw_output >= 0.0 || temperature_error >= 0.0;

        if not_undershooting && not_overshooting {
            self.stored_integral = raw_integral;
        }

        (proportional + self.stored_integral + derivative).clamp(0.0, 100.0)
    }

    fn current_interval_size(&self, now: Instant) -> Option<f64> {
        let last_update = self.last_update?;

        let elapsed_us = now.checked_duration_since(last_update)?.as_micros();

        if elapsed_us == 0 {
            return None;
        }

        Some(elapsed_us as f64 / 1_000_000.0)
    }
}
