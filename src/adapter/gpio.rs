use crate::domain::gpio::port::{InputDevice, OutputDevice, TemperatureSensor};

use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};
use onewire::{DS18B20, Device, OneWire};

const BLINK_DURATION_MS: u32 = 250;

pub struct Ds18b20Sensor<'d> {
    bus: OneWire<esp_hal::gpio::Flex<'d>>,
    sensor: DS18B20,
    last_temperature: f64,
}

impl<'d> Ds18b20Sensor<'d> {
    pub fn new(pin: impl esp_hal::gpio::Pin + 'd, address: [u8; 8]) -> Self {
        let mut pin = esp_hal::gpio::Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(esp_hal::gpio::DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        Self {
            bus: OneWire::new(pin, false),
            sensor: DS18B20::new(Device { address }).unwrap(),
            last_temperature: 0.0,
        }
    }
}

impl TemperatureSensor for Ds18b20Sensor<'_> {
    fn read_temperature(&mut self) -> f64 {
        let mut delay = Delay::new();
        let result = self
            .sensor
            .measure_temperature(&mut self.bus, &mut delay)
            .and_then(|resolution| {
                delay.delay_millis(resolution.time_ms() as u32);
                self.sensor.read_temperature(&mut self.bus, &mut delay)
            });

        if let Ok(raw_temperature) = result {
            self.last_temperature = raw_temperature as i16 as f64 / 16.0;
        }

        self.last_temperature
    }
}
pub struct EspInputDevice<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputDevice<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(pull));

        Self { pin }
    }
}

impl InputDevice for EspInputDevice<'_> {
    fn powered_state(&mut self) -> bool {
        self.pin.is_high()
    }
}

pub struct EspOutputDevice<'d> {
    pin: Output<'d>,
}

impl<'d> EspOutputDevice<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        let pin = Output::new(pin, Level::Low, OutputConfig::default());

        Self { pin }
    }

    fn invert_state(&mut self) {
        if self.pin.is_set_high() {
            self.pin.set_low();
        } else {
            self.pin.set_high();
        }
    }
}

impl OutputDevice for EspOutputDevice<'_> {
    fn blink_once(&mut self) {
        self.invert_state();

        Delay::new().delay_millis(BLINK_DURATION_MS);

        self.invert_state();
    }

    fn set_active(&mut self, active: bool) {
        if active {
            self.pin.set_high();
        } else {
            self.pin.set_low();
        }
    }
}
