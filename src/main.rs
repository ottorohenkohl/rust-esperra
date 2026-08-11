#![no_std]
#![no_main]

extern crate alloc;

use crate::adapter::gpio::EspOutputPort;

use adapter::config::FlashConfigPort;
use adapter::sensor::Ds18b20TemperaturePort;
use alloc::sync::Arc;
use domain::boiler::actor::BoilerActor;
use domain::boiler::protocol::BoilerMessage;
use domain::gpio::actor::TemperatureSensorActor;
use embassy_executor::Spawner;
use domain::gpio::protocol::TemperatureSensorMessage;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use heapless::Vec;
use library::actor::mailbox::Mailbox;

use embassy_futures::join::join;

mod adapter;
mod domain;
mod library;

const TICK_INTERVALL: Duration = Duration::from_millis(100);

static BOILER_ACTOR_MAILBOX: Mailbox<BoilerMessage, 8> = Mailbox::new();
static TEMPERATURE_SENSOR_ACTOR_MAILBOX: Mailbox<TemperatureSensorMessage, 1> = Mailbox::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let temperature_port = Ds18b20TemperaturePort::new(peripherals.GPIO4).unwrap();
    let heating_output_port = EspOutputPort::new(peripherals.GPIO10);
    let config_port = Arc::new(FlashConfigPort::new(peripherals.FLASH).unwrap());

    let boiler_ref = BOILER_ACTOR_MAILBOX.actor_ref();

    let boiler_actor = BoilerActor::new(config_port, heating_output_port);
    let temperature_sensor_actor = TemperatureSensorActor::new(temperature_port, Vec::from_slice(&[boiler_ref.subscriber()]).unwrap());

    join(join(BOILER_ACTOR_MAILBOX.run(boiler_actor), TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor)), event_loop()).await;
}

async fn event_loop() {
    let boiler_actor = BOILER_ACTOR_MAILBOX.actor_ref();
    let temperature_sensor_actor = TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref();


    loop {
        boiler_actor.tell(BoilerMessage::SwitchHeater);
        temperature_sensor_actor.tell(TemperatureSensorMessage::MeasureTemperature);

        Timer::after(TICK_INTERVALL).await;
    }
}
