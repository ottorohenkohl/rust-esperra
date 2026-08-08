use crate::domain::gpio::port::{InputDevice, OutputDevice};

use core::cell::RefCell;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull},
};

const BLINK_DURATION_MS: u32 = 250;

pub struct EspInputDevice<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputDevice<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        Self {
            pin: Input::new(pin, InputConfig::default().with_pull(pull)),
        }
    }
}

impl InputDevice for EspInputDevice<'_> {
    fn powered_state(&self) -> bool {
        self.pin.is_high()
    }
}

pub struct EspOutputDevice<'d> {
    pin: RefCell<Output<'d>>,
}

impl<'d> EspOutputDevice<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        Self {
            pin: RefCell::new(Output::new(pin, Level::Low, OutputConfig::default())),
        }
    }
}

impl OutputDevice for EspOutputDevice<'_> {
    fn blink_once(&self) {
        let mut pin = self.pin.borrow_mut();
        pin.set_high();
        Delay::new().delay_millis(BLINK_DURATION_MS);
        pin.set_low();
    }

    fn set_active(&self, active: bool) {
        let mut pin = self.pin.borrow_mut();
        if active {
            pin.set_high();
        } else {
            pin.set_low();
        }
    }
}
