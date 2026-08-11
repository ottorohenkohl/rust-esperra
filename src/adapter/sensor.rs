use crate::domain::gpio::sensor::TemperaturePort;
use crate::domain::gpio::sensor::TemperatureReadError;
use embassy_time::Duration;
use embassy_time::Timer;
use esp_hal::delay::Delay;
use esp_hal::gpio::DriveMode;
use esp_hal::gpio::Flex;
use esp_hal::gpio::OutputConfig;
use esp_hal::gpio::Pin;
use onewire::DS18B20;
use onewire::DeviceSearch;
use onewire::OneWire;

pub struct Ds18b20TemperaturePort<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: DS18B20,
}

impl<'d> Ds18b20TemperaturePort<'d> {
    pub fn new(pin: impl Pin + 'd) -> Result<Self, TemperatureReadError> {
        let mut pin = Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        let mut bus = OneWire::new(pin, false);
        let mut delay = Delay::new();
        let device = bus.search_next(&mut DeviceSearch::new_for_family(0x28), &mut delay).map_err(|_| TemperatureReadError::MeasurementFailed)?.ok_or(TemperatureReadError::MeasurementFailed)?;
        let sensor = DS18B20::new(device).map_err(|_| TemperatureReadError::MeasurementFailed)?;

        Ok(Self { bus, sensor })
    }
}

impl TemperaturePort for Ds18b20TemperaturePort<'_> {
    async fn read_temperature(&mut self) -> Result<f64, TemperatureReadError> {
        let mut delay = Delay::new();
        let resolution = self.sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| TemperatureReadError::MeasurementFailed)?;
        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;
        let raw_temperature = self.sensor.read_temperature(&mut self.bus, &mut delay).map_err(|_| TemperatureReadError::MeasurementFailed)?;
        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
