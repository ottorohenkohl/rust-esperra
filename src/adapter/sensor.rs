use crate::domain::gpio::sensor::TemperaturePort;
use crate::library::error::Error;
use embassy_time::{Duration, Timer};
use esp_hal::delay::Delay;
use esp_hal::gpio::{DriveMode, Flex, OutputConfig, Pin};
use onewire::{DS18B20, DeviceSearch, OneWire};

pub struct Ds18b20TemperaturePort<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: DS18B20,
}

impl<'d> Ds18b20TemperaturePort<'d> {
    pub fn new(pin: impl Pin + 'd) -> Result<Self, Error> {
        let mut pin = Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        let mut bus = OneWire::new(pin, false);
        let mut delay = Delay::new();
        let device = bus.search_next(&mut DeviceSearch::new_for_family(0x28), &mut delay).map_err(|_| Error)?.ok_or(Error)?;
        let sensor = DS18B20::new(device).map_err(|_| Error)?;

        Ok(Self { bus, sensor })
    }
}

impl TemperaturePort for Ds18b20TemperaturePort<'_> {
    async fn read_temperature(&mut self) -> Result<f64, Error> {
        let mut delay = Delay::new();
        let resolution = self.sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| Error)?;
        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;
        let raw_temperature = self.sensor.read_temperature(&mut self.bus, &mut delay).map_err(|_| Error)?;
        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
