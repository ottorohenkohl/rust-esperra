use crate::domain::gpio::port::{InputPort, OutputPort, TemperatureSensorError, TemperatureSensorPort};

use embassy_time::{Duration, Timer};
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull, DriveMode, Flex, Pin};
use onewire::{DeviceSearch, OneWire};

use onewire::DS18B20;

pub struct EspOutputAdapter<'d> {
    pin: Output<'d>,
}

impl<'d> EspOutputAdapter<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        let pin = Output::new(pin, Level::Low, OutputConfig::default());

        Self { pin }
    }
}

impl OutputPort for EspOutputAdapter<'_> {
    async fn set_active(&mut self, active: bool) {
        if active {
            self.pin.set_high();
        } else {
            self.pin.set_low();
        }
    }

    async fn invert_state(&mut self) -> bool {
        let active = self.pin.is_set_low();

        self.set_active(active).await;

        active
    }
}

pub struct EspInputAdapter<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputAdapter<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(pull));

        Self { pin }
    }
}

impl InputPort for EspInputAdapter<'_> {
    fn powered_state(&self) -> bool {
        self.pin.is_low()
    }
}

pub struct Ds18b20TemperatureAdapter<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: Option<DS18B20>,
}

impl<'d> Ds18b20TemperatureAdapter<'d> {
    pub fn new(pin: impl Pin + 'd) -> Self {
        let mut pin = Flex::new(pin);

        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        let mut bus = OneWire::new(pin, false);
        let sensor = Self::discover(&mut bus).ok();

        Self { bus, sensor }
    }


    fn discover(bus: &mut OneWire<Flex<'d>>) -> Result<DS18B20, TemperatureSensorError> {
        let mut delay = Delay::new();

        let device = bus.search_next(&mut DeviceSearch::new_for_family(0x28), &mut delay).map_err(|_| TemperatureSensorError::DiscoveryFailed)?.ok_or(TemperatureSensorError::NotFound)?;

        DS18B20::new(device).map_err(|_| TemperatureSensorError::InitializationFailed)
    }
}

impl TemperatureSensorPort for Ds18b20TemperatureAdapter<'_> {
    async fn read_temperature(&mut self) -> Result<f64, TemperatureSensorError> {
        let mut delay = Delay::new();

        if self.sensor.is_none() {
            self.sensor = Some(Self::discover(&mut self.bus)?);
        }

        let sensor = self.sensor.as_ref().unwrap();

        let resolution = sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| TemperatureSensorError::MeasurementStartFailed)?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let raw_temperature = sensor.read_temperature(&mut self.bus, &mut delay).map_err(|_| TemperatureSensorError::MeasurementReadFailed)?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
