use crate::domain::gpio::port::{InputDevice, OutputDevice};

use core::cell::RefCell;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin},
};

pub struct EspInputDevice<'d> {
    pin: Input<'d>,
    powered_level: Level,
}

impl<'d> EspInputDevice<'d> {
    pub fn new(pin: impl InputPin + 'd, config: InputConfig, powered_level: Level) -> Self {
        Self {
            pin: Input::new(pin, config),
            powered_level,
        }
    }
}

impl InputDevice for EspInputDevice<'_> {
    fn powered_state(&self) -> bool {
        match self.powered_level {
            Level::High => self.pin.is_high(),
            Level::Low => self.pin.is_low(),
        }
    }
}

pub struct EspOutputDevice<'d> {
    pin: RefCell<Output<'d>>,
    active_level: Level,
    blink_duration_ms: u32,
}

impl<'d> EspOutputDevice<'d> {
    pub fn new(
        pin: impl OutputPin + 'd,
        config: OutputConfig,
        active_level: Level,
        blink_duration_ms: u32,
    ) -> Self {
        Self {
            pin: RefCell::new(Output::new(pin, Self::inactive_level(active_level), config)),
            active_level,
            blink_duration_ms,
        }
    }

    fn inactive_level(active_level: Level) -> Level {
        match active_level {
            Level::High => Level::Low,
            Level::Low => Level::High,
        }
    }

    fn set_level(&self, level: Level) {
        let mut pin = self.pin.borrow_mut();
        match level {
            Level::High => pin.set_high(),
            Level::Low => pin.set_low(),
        }
    }
}

impl OutputDevice for EspOutputDevice<'_> {
    fn blink_once(&self) {
        self.set_level(self.active_level);
        let delay = Delay::new();
        delay.delay_millis(self.blink_duration_ms);
        self.set_level(Self::inactive_level(self.active_level));
    }

    fn set_active(&self, active: bool) {
        let level = if active {
            self.active_level
        } else {
            Self::inactive_level(self.active_level)
        };
        self.set_level(level);
    }
}
