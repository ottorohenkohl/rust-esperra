use crate::domain::gpio::port::InputPort;
use crate::domain::gpio::port::OutputPort;

use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

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
        self.set_active(self.pin.is_set_low());
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
    fn powered_state(&mut self) -> bool {
        self.pin.is_high()
    }
}
