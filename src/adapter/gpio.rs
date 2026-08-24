use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::port::OutputPort;
use crate::domain::gpio::port::TemperatureSensorPort;
use crate::library::error::Error;

use embassy_time::{Duration, Timer};
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull, DriveMode, Flex, Pin};
use onewire::{DeviceSearch, OneWire};

use onewire::DS18B20;

pub struct EspOutputPort<'d> {
    pin: Output<'d>,
}

impl<'d> EspOutputPort<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        let pin = Output::new(pin, Level::Low, OutputConfig::default());

        Self { pin }
    }
}

impl OutputPort for EspOutputPort<'_> {
    async fn set_active(&mut self, active: bool) {
        if active {
            self.pin.set_high();
        } else {
            self.pin.set_low();
        }
    }

    async fn invert_state(&mut self) {
        let _ = self.set_active(self.pin.is_set_low());
    }
}

pub struct EspInputPort<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputPort<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(pull));

        Self { pin }
    }
}

impl InputPort for EspInputPort<'_> {
    fn powered_state(&self) -> bool {
        self.pin.is_high()
    }
}

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

impl TemperatureSensorPort for Ds18b20TemperaturePort<'_> {
    async fn read_temperature(&mut self) -> Result<f64, Error> {
        let mut delay = Delay::new();

        let resolution = self.sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| Error)?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let raw_temperature = self.sensor.read_temperature(&mut self.bus, &mut delay).map_err(|_| Error)?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
