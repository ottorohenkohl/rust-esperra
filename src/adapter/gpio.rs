use crate::domain::{gpio::port::InputDevice, output::OutputMessage};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

const BLINK_DURATION: Duration = Duration::from_millis(250);

pub struct LedActor;

impl LedActor {
    pub async fn run<'d>(
        pin: impl OutputPin + 'd,
        messages: Receiver<'_, CriticalSectionRawMutex, OutputMessage, 1>,
    ) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());
        let mut blinking = false;

        loop {
            match messages.receive().await {
                OutputMessage::BlinkOnce if !blinking => {
                    blinking = true;
                    pin.set_high();
                    Timer::after(BLINK_DURATION).await;
                    pin.set_low();
                    blinking = false;
                }
                OutputMessage::SetActive(active) => {
                    if active {
                        pin.set_high();
                    } else {
                        pin.set_low();
                    }
                }
                OutputMessage::BlinkOnce => {}
            }
        }
    }
}

pub struct HeatingElementActor;

impl HeatingElementActor {
    pub async fn run<'d>(
        pin: impl OutputPin + 'd,
        messages: Receiver<'_, CriticalSectionRawMutex, OutputMessage, 1>,
    ) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            match messages.receive().await {
                OutputMessage::SetActive(active) => {
                    if active {
                        pin.set_high();
                    } else {
                        pin.set_low();
                    }
                }
                OutputMessage::BlinkOnce => {}
            }
        }
    }
}

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
    fn powered_state(&mut self) -> bool {
        self.pin.is_high()
    }
}
