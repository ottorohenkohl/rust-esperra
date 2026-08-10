use crate::domain::gpio::output::LightMessage;
use crate::domain::gpio::output::OutputMessage;
use crate::domain::gpio::port::InputPort;
use crate::library::actor::Actor;
use crate::library::actor::AsyncActor;
use embassy_time::Duration;
use embassy_time::Timer;
use esp_hal::gpio::Input;
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::InputPin;
use esp_hal::gpio::Level;
use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::gpio::OutputPin;
use esp_hal::gpio::Pull;

const BLINK_DURATION: Duration = Duration::from_millis(250);

pub struct LightActor<'d> {
    pin: Output<'d>,
}

impl<'d> LightActor<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        Self { pin: Output::new(pin, Level::Low, OutputConfig::default()) }
    }
}

impl AsyncActor for LightActor<'_> {
    type Message = LightMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => set_output(&mut self.pin, active),
            LightMessage::BlinkOnce => {
                self.pin.set_high();
                Timer::after(BLINK_DURATION).await;
                self.pin.set_low();
            }
        }
    }
}

pub struct HeatingElementActor<'d> {
    pin: Output<'d>,
}

impl<'d> HeatingElementActor<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        Self { pin: Output::new(pin, Level::Low, OutputConfig::default()) }
    }
}

impl Actor for HeatingElementActor<'_> {
    type Message = OutputMessage;

    fn handle(&mut self, message: Self::Message) {
        let OutputMessage::SetActive(active) = message;
        set_output(&mut self.pin, active);
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
        Self { pin: Input::new(pin, InputConfig::default().with_pull(pull)) }
    }
}

impl InputPort for EspInputPort<'_> {
    fn powered_state(&mut self) -> bool {
        self.pin.is_high()
    }
}
