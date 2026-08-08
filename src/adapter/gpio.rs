use crate::domain::gpio::port::{InputDevice, OutputDevice};

use core::cell::RefCell;
use critical_section::Mutex;
use embassy_futures::select::{select, Either};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, TrySendError},
    signal::Signal,
};
use embassy_time::{Duration, Timer};
use esp_hal::{
    gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin},
};

const BLINK_DURATION: Duration = Duration::from_millis(250);

enum OutputCommand {
    Blink,
}

pub struct OutputResources {
    commands: Channel<CriticalSectionRawMutex, OutputCommand, 1>,
    active_changed: Signal<CriticalSectionRawMutex, ()>,
    active: Mutex<RefCell<bool>>,
    blinking: Mutex<RefCell<bool>>,
}

impl OutputResources {
    pub const fn new() -> Self {
        Self {
            commands: Channel::new(),
            active_changed: Signal::new(),
            active: Mutex::new(RefCell::new(false)),
            blinking: Mutex::new(RefCell::new(false)),
        }
    }

    fn active(&self) -> bool {
        critical_section::with(|cs| *self.active.borrow(cs).borrow())
    }
}

pub struct EspInputDevice<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputDevice<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: esp_hal::gpio::Pull) -> Self {
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

pub struct EspOutputDevice {
    resources: &'static OutputResources,
}

impl EspOutputDevice {
    pub const fn new(resources: &'static OutputResources) -> Self {
        Self { resources }
    }

    pub async fn run<'d>(&self, pin: impl OutputPin + 'd) -> ! {
        let mut pin = Output::new(pin, Level::Low, OutputConfig::default());

        loop {
            match select(
                self.resources.commands.receive(),
                self.resources.active_changed.wait(),
            )
            .await
            {
                Either::First(OutputCommand::Blink) => {
                    pin.set_high();
                    Timer::after(BLINK_DURATION).await;
                    if self.resources.active() {
                        pin.set_high();
                    } else {
                        pin.set_low();
                    }
                    critical_section::with(|cs| {
                        *self.resources.blinking.borrow(cs).borrow_mut() = false;
                    });
                }
                Either::Second(()) => {
                    if self.resources.active() {
                        pin.set_high();
                    } else {
                        pin.set_low();
                    }
                }
            }
        }
    }
}

impl OutputDevice for EspOutputDevice {
    fn blink_once(&self) {
        let should_blink = critical_section::with(|cs| {
            let mut blinking = self.resources.blinking.borrow(cs).borrow_mut();
            if *blinking {
                false
            } else {
                *blinking = true;
                true
            }
        });

        if should_blink
            && matches!(
                self.resources.commands.try_send(OutputCommand::Blink),
                Err(TrySendError::Full(_))
            )
        {
            critical_section::with(|cs| {
                *self.resources.blinking.borrow(cs).borrow_mut() = false;
            });
        }
    }

    fn set_active(&self, active: bool) {
        critical_section::with(|cs| {
            *self.resources.active.borrow(cs).borrow_mut() = active;
        });
        self.resources.active_changed.signal(());
    }
}
