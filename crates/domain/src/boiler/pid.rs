use crate::config::port::PidGains;

pub struct PidService {
    pid_gains: PidGains,
    interval_ms: u64,
    last_temperature: f64,
    stored_integral: f64,
}

impl PidService {
    pub fn new(pid_gains: PidGains, interval_ms: u64) -> Self {
        Self {
            pid_gains,
            interval_ms,
            last_temperature: 0.0,
            stored_integral: 0.0,
        }
    }

    pub fn compute(&mut self, target_temperature: f64, actual_temperature: f64) -> f64 {
        let interval_size = (self.interval_ms as f64 / 1000.0).max(f64::EPSILON);
        let temperature_error = target_temperature - actual_temperature;
        let temperature_change = self.last_temperature - actual_temperature;

        let proportional = self.pid_gains.k_proportional * temperature_error;
        let derivative = self.pid_gains.k_derivative * temperature_change / interval_size;
        let raw_integral =
            self.pid_gains.k_intergral * temperature_error * interval_size + self.stored_integral;
        let raw_output = proportional + raw_integral + derivative;

        self.last_temperature = actual_temperature;

        let not_undershooting = raw_output <= 100.0 || temperature_error <= 0.0;
        let not_overshooting = raw_output >= 0.0 || temperature_error >= 0.0;
        if not_undershooting && not_overshooting {
            self.stored_integral = raw_integral;
        }

        (proportional + self.stored_integral + derivative).clamp(0.0, 100.0)
    }
}
