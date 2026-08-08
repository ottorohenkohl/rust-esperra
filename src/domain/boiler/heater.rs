use crate::domain::clock::port::Instant;
use crate::domain::config::port::ConfigProvider;
use crate::domain::gpio::port::{OutputDevice, TemperatureSensor};

use super::pid::Pid;

use alloc::boxed::Box;
use alloc::sync::Arc;

struct Heater {
    config_provider: Arc<dyn ConfigProvider>,
    cycle_start: Box<dyn Instant>,
    heating_element: Arc<dyn OutputDevice>,
    temperature_sensor: Arc<dyn TemperatureSensor>,

    local_pid: Pid,
}

impl Heater {
    pub fn new(cycle_start: Box<dyn Instant>, config_provider: Arc<dyn ConfigProvider>, heating_element: Arc<dyn OutputDevice>, temperature_sensor: Arc<dyn TemperatureSensor>) -> Self {
        let local_pid = Pid::new(Arc::clone(&config_provider));

        Self { cycle_start, config_provider, heating_element, temperature_sensor, local_pid }
    }

    pub fn switch(&mut self) {
        let heater_options = self.config_provider.read_heater_options();
        let time_elapsed = self.cycle_start.elapsed_ms();
        let actual_temperature = self.temperature_sensor.read_temperature();
        let pid_percentage = self.local_pid.compute(heater_options.target_temperature, actual_temperature);

        let should_heat = if actual_temperature >= heater_options.target_temperature - heater_options.boost_intervall_size { pid_percentage < time_elapsed as f64 / heater_options.heating_cycle_ms as f64 } else { true };

        self.heating_element.set_active(should_heat);
    }
}
