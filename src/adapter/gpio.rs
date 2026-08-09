use crate::domain::gpio::port::InputDevice;

use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull};

const BLINK_DURATION: Duration = Duration::from_millis(250);

enum LedCommand {
    Blink,
}

pub struct LedActorResources {
    commands: Channel<CriticalSectionRawMutex, LedCommand, 1>,
    blinking: Mutex<RefCell<bool>>,
}

impl LedActorResources {
    pub const fn new() -> Self {
        Self {
            commands: Channel::new(),
            blinking: Mutex::new(RefCell::new(false)),
        }
    }
}

pub struct LedActor {
    resources: &'static LedActorResources,
}

impl LedActor {
    pub const fn new(resources: &'static LedActorResources) -> Self {
        Self { resources }
    }

    pub fn blink_once(&self) {
        let should_blink = critical_section::with(|cs| {
            let mut blinking = self.resources.blinking.borrow(cs).borrow_mut();
            if *blinking {
                false
            } else {
                *blinking = true;
                true
            }
        });

        if should_blink && self.resources.commands.try_send(LedCommand::Blink).is_err() {
            critical_section::with(|cs| {
                *self.resources.blinking.borrow(cs).borrow_mut() = false;
            });
        }
    }

    pub async fn run<'d>(&self, pin: impl OutputPin + 'd) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            let LedCommand::Blink = self.resources.commands.receive().await;
            pin.set_high();
            Timer::after(BLINK_DURATION).await;
            pin.set_low();
            critical_section::with(|cs| {
                *self.resources.blinking.borrow(cs).borrow_mut() = false;
            });
        }
    }
}

pub struct HeatingElement<'d> {
    pin: Output<'d>,
}

impl<'d> HeatingElement<'d> {
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        Self {
            pin: Output::new(pin, Level::Low, OutputConfig::default()),
        }
    }

    pub fn set_active(&mut self, active: bool) {
        if active {
            self.pin.set_high();
        } else {
            self.pin.set_low();
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
