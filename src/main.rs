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
use domain::gpio::actor::TemperatureSensorActor;
use domain::gpio::protocol::{OutputMessage, TemperatureSensorMessage};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use library::actor::mailbox::Mailbox;

mod adapter;
mod domain;
mod library;

const TEMPERATURE_MEASUREMENT_INTERVAL: Duration = Duration::from_secs(1);

static BOILER_ACTOR_MAILBOX: Mailbox<BoilerMessage> = Mailbox::new();
static TEMPERATURE_SENSOR_ACTOR_MAILBOX: Mailbox<TemperatureSensorMessage> = Mailbox::new();
static HEATING_OUTPUT_ACTOR_MAILBOX: Mailbox<OutputMessage> = Mailbox::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let boiler_ref = BOILER_ACTOR_MAILBOX.actor_ref();
    let heating_element_ref = HEATING_OUTPUT_ACTOR_MAILBOX.actor_ref();

    let config_port = Arc::new(DefaultConfigPort::new());
    let boiler_actor = BoilerActor::new(config_port, heating_element_ref);
    let heating_output_actor = HeatingElementActor::new(peripherals.GPIO5);

    let temperature_port = Ds18b20TemperaturePort::new(peripherals.GPIO4).unwrap();

    let mut temperature_sensor_actor = TemperatureSensorActor::new(temperature_port);

    assert!(temperature_sensor_actor.subscribe(boiler_ref.subscriber()).is_ok());

    join(join3(BOILER_ACTOR_MAILBOX.run(boiler_actor), TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor), HEATING_OUTPUT_ACTOR_MAILBOX.run(heating_output_actor)), event_loop()).await;
}

async fn event_loop() {
    let temperature_sensor = TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref();

    loop {
        temperature_sensor.tell(TemperatureSensorMessage::MeasureTemperature);

        Timer::after(TEMPERATURE_MEASUREMENT_INTERVAL).await;
    }
}
