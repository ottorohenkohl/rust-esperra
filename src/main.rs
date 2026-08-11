#![no_std]
#![no_main]

extern crate alloc;

use adapter::config::DefaultConfigPort;
use adapter::gpio::HeatingElementActor;
use adapter::sensor::Ds18b20TemperaturePort;
use alloc::sync::Arc;
use core::panic::PanicInfo;
use domain::boiler::actor::BoilerActor;
use domain::boiler::protocol::BoilerMessage;
use domain::gpio::actor::TemperatureActor;
use domain::gpio::protocol::{OutputMessage, TemperatureSensorMessage};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use library::actor::{ActorRef, Mailbox};

mod adapter;
mod domain;
mod library;

const TEMPERATURE_MEASUREMENT_INTERVAL: Duration = Duration::from_secs(1);

static BOILER_MAILBOX: Mailbox<BoilerMessage, 4> = Mailbox::new();
static SENSOR_MAILBOX: Mailbox<TemperatureSensorMessage, 1> = Mailbox::new();
static HEATING_ELEMENT_MAILBOX: Mailbox<OutputMessage, 8> = Mailbox::new();

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
    let config = Arc::new(DefaultConfigPort::new());
    let boiler = BoilerActor::new(config, heating_element_ref);
    let temperature_port = Ds18b20TemperaturePort::new(peripherals.GPIO4).unwrap();
    let mut temperature_sensor = TemperatureActor::new(temperature_port);
    assert!(temperature_sensor.subscribe(boiler_ref.subscriber()).is_ok());
    let heating_element = HeatingElementActor::new(peripherals.GPIO5);

    join3(BOILER_MAILBOX.run(boiler), SENSOR_MAILBOX.run(temperature_sensor), join(HEATING_ELEMENT_MAILBOX.run(heating_element), measure_temperature(&temperature_sensor_ref))).await;

    unreachable!()
}

async fn measure_temperature(temperature_sensor: &ActorRef<TemperatureSensorMessage>) -> ! {
    loop {
        temperature_sensor.tell(TemperatureSensorMessage::MeasureTemperature);
        Timer::after(TEMPERATURE_MEASUREMENT_INTERVAL).await;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
