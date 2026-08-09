use esperra_domain::{
    gpio::port::InputPort,
    output::{OutputMessage, OutputState, StatusLightMessage},
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

const BLINK_DURATION: Duration = Duration::from_millis(250);

pub struct LedActor;

impl LedActor {
    pub async fn run<'d, const N: usize>(
        pin: impl OutputPin + 'd,
        messages: Receiver<'_, CriticalSectionRawMutex, StatusLightMessage, N>,
    ) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            match messages.receive().await {
                StatusLightMessage::BlinkOnce => {
                    pin.set_high();
                    Timer::after(BLINK_DURATION).await;
                    pin.set_low();
                }
                StatusLightMessage::SetState(state) => set_output(&mut pin, state),
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
            let OutputMessage::SetState(state) = messages.receive().await;
            set_output(&mut pin, state);
        }
    }
}

fn set_output(pin: &mut Output<'_>, state: OutputState) {
    match state {
        OutputState::Active => pin.set_high(),
        OutputState::Inactive => pin.set_low(),
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
