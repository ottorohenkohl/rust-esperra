use crate::domain::gpio::port::{InputDevice, OutputDevice};

use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

const BLINK_DURATION_MS: u32 = 250;

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
