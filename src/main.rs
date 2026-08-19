#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;

use crate::domain::brewing::actor::BrewActor;
use crate::domain::controls::actor::CircuitActor;
use crate::domain::controls::protocol::CircuitMessage;
use crate::{adapter::clock::EspTimerPort, domain::brewing::protocol::BrewMessage};
use crate::adapter::gpio::{EspInputPort, EspOutputPort};
use crate::domain::gpio::actor::LightActor;
use crate::domain::gpio::protocol::LightMessage;

use adapter::config::FlashConfigPort;
use adapter::gpio::Ds18b20TemperaturePort;
use alloc::sync::Arc;
use domain::boiler::actor::BoilerActor;
use domain::config::port::ConfigPort;
use domain::boiler::protocol::BoilerMessage;
use domain::gpio::actor::TemperatureSensorActor;
use domain::gpio::protocol::TemperatureSensorMessage;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::gpio::Pull;
use esp_hal::timer::timg::TimerGroup;
use esp_storage::FlashStorage;
use heapless::Vec;
use library::actor::mailbox::Mailbox;

use embassy_futures::join::join3;

mod adapter;
mod domain;
mod library;
mod update;

const TICK_INTERVALL: Duration = Duration::from_millis(100);

static BOILER_ACTOR_MAILBOX: Mailbox<BoilerMessage, 8> = Mailbox::new();
static BREW_ACTOR_MAILBOX: Mailbox<BrewMessage, 8> = Mailbox::new();
static CIRCUIT_ACTOR_MAILBOX: Mailbox<CircuitMessage, 8> = Mailbox::new();
static STATUS_LIGHT_ACTOR_MAILBOX: Mailbox<LightMessage, 1> = Mailbox::new();
static POWER_LIGHT_ACTOR_MAILBOX: Mailbox<LightMessage, 1> = Mailbox::new();
static TEMPERATURE_SENSOR_ACTOR_MAILBOX: Mailbox<TemperatureSensorMessage, 1> = Mailbox::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let mut flash = FlashStorage::new(peripherals.FLASH);
    update::initialize(&mut flash);
    let config_port = Arc::new(FlashConfigPort::new(&mut flash).unwrap());
    let firmware_info = config_port.read_firmware_info().clone();
    let heating_output_port = EspOutputPort::new(peripherals.GPIO10);
    let status_light_output_port = EspOutputPort::new(peripherals.GPIO11);
    let power_light_output_port = EspOutputPort::new(peripherals.GPIO12);
    let pump_output_port = EspOutputPort::new(peripherals.GPIO13);
    let temperature_port = Ds18b20TemperaturePort::new(peripherals.GPIO4).unwrap();
    let valve_output_port = EspOutputPort::new(peripherals.GPIO14);

    let power_input_port = EspInputPort::new(peripherals.GPIO15, Pull::Up);
    let status_input_port = EspInputPort::new(peripherals.GPIO16, Pull::Up);
    let water_input_port = EspInputPort::new(peripherals.GPIO17, Pull::Up);

    let boiler_actor = BoilerActor::new(config_port, heating_output_port, POWER_LIGHT_ACTOR_MAILBOX.actor_ref(), STATUS_LIGHT_ACTOR_MAILBOX.actor_ref());
    let brew_actor = BrewActor::new(pump_output_port, valve_output_port);
    let circuit_actor = CircuitActor::new(BOILER_ACTOR_MAILBOX.actor_ref(), BREW_ACTOR_MAILBOX.actor_ref(), power_input_port, status_input_port, STATUS_LIGHT_ACTOR_MAILBOX.actor_ref(), water_input_port);
    let power_light_actor = LightActor::new(power_light_output_port, EspTimerPort::new());
    let status_light_actor = LightActor::new(status_light_output_port, EspTimerPort::new());
    let temperature_sensor_actor = TemperatureSensorActor::new(temperature_port, Vec::from_slice(&[BOILER_ACTOR_MAILBOX.actor_ref().subscriber()]).unwrap());

    let actor_group_1 = join3(BOILER_ACTOR_MAILBOX.run(boiler_actor), BREW_ACTOR_MAILBOX.run(brew_actor), CIRCUIT_ACTOR_MAILBOX.run(circuit_actor));
    let actor_group_2 = join3(POWER_LIGHT_ACTOR_MAILBOX.run(power_light_actor), STATUS_LIGHT_ACTOR_MAILBOX.run(status_light_actor), TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor));

    update::start(
        peripherals.WIFI,
        peripherals.LPWR,
        peripherals.RNG,
        peripherals.ADC1,
        flash,
        firmware_info,
        BOILER_ACTOR_MAILBOX.actor_ref(),
        BREW_ACTOR_MAILBOX.actor_ref(),
        spawner,
    );

    join3(actor_group_1, actor_group_2, event_loop()).await;
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
