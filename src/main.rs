#![no_std]
#![no_main]

extern crate alloc;

use adapter::config::DefaultConfigPort;
use adapter::gpio::HeatingElementActor;
use adapter::sensor::TemperatureSensorActor;
use alloc::sync::Arc;
use core::panic::PanicInfo;
use domain::boiler::BoilerActor;
use domain::boiler::BoilerMessage;
use domain::gpio::output::OutputMessage;
use domain::gpio::sensor::TemperatureSensorMessage;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::join::join3;
use embassy_time::Duration;
use embassy_time::Timer;
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use library::actor::ActorRef;
use library::actor::Mailbox;

mod adapter;
mod domain;
mod library;

const BOILER_INTERVAL: Duration = Duration::from_secs(1);

static BOILER_MAILBOX: Mailbox<BoilerMessage, 4> = Mailbox::new();
static SENSOR_MAILBOX: Mailbox<TemperatureSensorMessage, 1> = Mailbox::new();
static HEATING_ELEMENT_MAILBOX: Mailbox<OutputMessage, 1> = Mailbox::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let boiler_ref = BOILER_MAILBOX.actor_ref();
    let temperature_sensor_ref = SENSOR_MAILBOX.actor_ref();
    let heating_element_ref = HEATING_ELEMENT_MAILBOX.actor_ref();
    let config: Arc<dyn domain::config::port::ConfigPort> = Arc::new(DefaultConfigPort::new());
    let boiler = BoilerActor::new(config, temperature_sensor_ref, heating_element_ref);
    let mut temperature_sensor = TemperatureSensorActor::new(peripherals.GPIO4).unwrap();
    assert!(temperature_sensor.subscribe(boiler_ref.subscriber()).is_ok());
    let heating_element = HeatingElementActor::new(peripherals.GPIO5);

    join3(BOILER_MAILBOX.run(boiler), SENSOR_MAILBOX.run(temperature_sensor), join(HEATING_ELEMENT_MAILBOX.run(heating_element), evaluate_boiler(&boiler_ref))).await;

    unreachable!()
}

async fn evaluate_boiler(boiler: &ActorRef<BoilerMessage>) -> ! {
    loop {
        boiler.tell(BoilerMessage::EvaluateHeating);
        Timer::after(BOILER_INTERVAL).await;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
