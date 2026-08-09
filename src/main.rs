#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

use adapter::{
    actor_ref::EmbassyActorRef, boiler::run_boiler_actor, clock::EmbassyInstant,
    config::DefaultConfigPort, gpio::HeatingElementActor, sensor::TemperatureSensorActor,
};
use domain::{
    boiler::{BoilerActor, BoilerMessage},
    gpio::{output::OutputMessage, sensor::TemperatureSensorMessage},
};
use embassy_executor::Spawner;
use embassy_futures::join::{join, join3};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use library::actor::ActorRef;

mod adapter;
mod domain;
mod library;

const BOILER_INTERVAL: Duration = Duration::from_secs(1);

static BOILER_MAILBOX: Channel<CriticalSectionRawMutex, BoilerMessage, 4> = Channel::new();
static SENSOR_MAILBOX: Channel<CriticalSectionRawMutex, TemperatureSensorMessage, 1> =
    Channel::new();
static HEATING_ELEMENT_MAILBOX: Channel<CriticalSectionRawMutex, OutputMessage, 1> = Channel::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_rtos::start(
        TimerGroup::new(peripherals.TIMG0).timer0,
        SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0,
    );

    let boiler_ref = EmbassyActorRef::new(BOILER_MAILBOX.sender());
    let temperature_sensor_ref = EmbassyActorRef::new(SENSOR_MAILBOX.sender());
    let heating_element_ref = EmbassyActorRef::new(HEATING_ELEMENT_MAILBOX.sender());
    let config = DefaultConfigPort::new();
    let mut boiler = BoilerActor::new(
        &config,
        temperature_sensor_ref,
        heating_element_ref,
        EmbassyInstant::now(),
    );
    let mut temperature_sensor = TemperatureSensorActor::new(peripherals.GPIO4).unwrap();

    join3(
        run_boiler_actor(&mut boiler, BOILER_MAILBOX.receiver()),
        temperature_sensor.run(SENSOR_MAILBOX.receiver(), &boiler_ref),
        join(
            HeatingElementActor::run(peripherals.GPIO5, HEATING_ELEMENT_MAILBOX.receiver()),
            evaluate_boiler(&boiler_ref),
        ),
    )
    .await;

    unreachable!()
}

async fn evaluate_boiler(boiler: &EmbassyActorRef<'_, BoilerMessage, 4>) -> ! {
    loop {
        boiler.send(BoilerMessage::EvaluateHeating);
        Timer::after(BOILER_INTERVAL).await;
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
