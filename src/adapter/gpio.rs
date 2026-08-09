use crate::domain::{
    gpio::output::{LightMessage, OutputMessage},
    gpio::port::InputPort,
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

const BLINK_DURATION: Duration = Duration::from_millis(250);

pub struct LightActor;

impl LightActor {
    pub async fn run<'d, const N: usize>(
        pin: impl OutputPin + 'd,
        messages: Receiver<'_, CriticalSectionRawMutex, LightMessage, N>,
    ) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            match messages.receive().await {
                LightMessage::BlinkOnce => {
                    pin.set_high();
                    Timer::after(BLINK_DURATION).await;
                    pin.set_low();
                }
                LightMessage::SetActive(active) => set_output(&mut pin, active),
            }
        }
    }
}

pub struct HeatingElementActor;

impl HeatingElementActor {
    pub async fn run<'d, const N: usize>(
        pin: impl OutputPin + 'd,
        messages: Receiver<'_, CriticalSectionRawMutex, OutputMessage, N>,
    ) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            let OutputMessage::SetActive(active) = messages.receive().await;
            set_output(&mut pin, active);
        }
    }
}

fn set_output(pin: &mut Output<'_>, active: bool) {
    if active {
        pin.set_high();
    } else {
        pin.set_low();
    }
}

pub struct EspInputPort<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputPort<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        Self {
            pin: Input::new(pin, InputConfig::default().with_pull(pull)),
        }
    }
}

impl InputPort for EspInputPort<'_> {
    fn powered_state(&mut self) -> bool {
        self.pin.is_high()
    }
}
