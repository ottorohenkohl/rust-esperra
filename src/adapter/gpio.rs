use crate::domain::gpio::output::LightPort;
use crate::domain::gpio::output::OutputMessage;
use crate::domain::gpio::port::InputPort;
use crate::library::actor::Actor;
use crate::library::error::Error;
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

pub struct GpioLightPort<'d> {
    pin: Output<'d>,
}

impl<'d> GpioLightPort<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        Self { pin: Output::new(pin, Level::Low, OutputConfig::default()) }
    }
}

impl LightPort for GpioLightPort<'_> {
    async fn set_active(&mut self, active: bool) -> Result<(), Error> {
        set_output(&mut self.pin, active);
        Ok(())
    }

    async fn blink(&mut self, duration: Duration) -> Result<(), Error> {
        self.pin.set_high();
        Timer::after(duration).await;
        self.pin.set_low();
        Ok(())
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

    async fn handle(&mut self, message: Self::Message) {
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
