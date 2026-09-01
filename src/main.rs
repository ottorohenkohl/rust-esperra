#![no_std]
#![no_main]

extern crate alloc;
extern crate esp_backtrace;

use crate::adapter::clock::EspTimerPort;
use crate::adapter::config::FlashConfigPort;
use crate::adapter::gpio::{Ds18b20TemperaturePort, EspInputPort, EspOutputPort};
use crate::adapter::update::EspUpdatePort;
use crate::domain::boiler::actor::BoilerActor;
use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::actor::BrewActor;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::controls::actor::{ButtonActor, WaterActor};
use crate::domain::controls::protocol::{ButtonMessage, WaterMessage};
use crate::domain::gpio::actor::{LightActor, TemperatureSensorActor};
use crate::domain::gpio::protocol::{LightMessage, TemperatureSensorMessage};
use crate::domain::update::actor::UpdateActor;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::mailbox::EmbassyMailbox;

use alloc::sync::Arc;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::Config;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Pull;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use esp_storage::FlashStorage;
use heapless::Vec;

use embassy_futures::join::join3;

mod adapter;
mod domain;
mod library;

const TICK_INTERVALL: Duration = Duration::from_millis(1000);

static BOILER_ACTOR_MAILBOX: EmbassyMailbox<BoilerMessage, 8> = EmbassyMailbox::new();
static BREW_ACTOR_MAILBOX: EmbassyMailbox<BrewMessage, 8> = EmbassyMailbox::new();
static BUTTON_ACTOR_MAILBOX: EmbassyMailbox<ButtonMessage, 8> = EmbassyMailbox::new();
static WATER_ACTOR_MAILBOX: EmbassyMailbox<WaterMessage, 8> = EmbassyMailbox::new();
static STATUS_LIGHT_ACTOR_MAILBOX: EmbassyMailbox<LightMessage, 1> = EmbassyMailbox::new();
static POWER_LIGHT_ACTOR_MAILBOX: EmbassyMailbox<LightMessage, 1> = EmbassyMailbox::new();
static TEMPERATURE_SENSOR_ACTOR_MAILBOX: EmbassyMailbox<TemperatureSensorMessage, 1> = EmbassyMailbox::new();
static UPDATE_ACTOR_MAILBOX: EmbassyMailbox<UpdateMessage, 1> = EmbassyMailbox::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let mut flash = FlashStorage::new(peripherals.FLASH);

    EspUpdatePort::<FlashConfigPort>::initialize(&mut flash).unwrap();

    let config_port = Arc::new(FlashConfigPort::new(&mut flash).unwrap());
    let heating_output_port = EspOutputPort::new(peripherals.GPIO11);
    let status_light_output_port = EspOutputPort::new(peripherals.GPIO12);
    let power_light_output_port = EspOutputPort::new(peripherals.GPIO13);
    let pump_output_port = EspOutputPort::new(peripherals.GPIO14);
    let temperature_port = Ds18b20TemperaturePort::new(peripherals.GPIO4).unwrap();
    let valve_output_port = EspOutputPort::new(peripherals.GPIO10);
    let power_input_port = EspInputPort::new(peripherals.GPIO20, Pull::Up);
    let status_input_port = EspInputPort::new(peripherals.GPIO19, Pull::Up);
    let water_input_port = EspInputPort::new(peripherals.GPIO21, Pull::Up);
    let update_port = EspUpdatePort::new(peripherals.WIFI, peripherals.LPWR, peripherals.RNG, peripherals.ADC1, flash, Arc::clone(&config_port), spawner);

    let boiler_actor_ref = BOILER_ACTOR_MAILBOX.actor_ref();
    let brew_actor_ref = BREW_ACTOR_MAILBOX.actor_ref();
    let button_actor_ref = BUTTON_ACTOR_MAILBOX.actor_ref();
    let water_actor_ref = WATER_ACTOR_MAILBOX.actor_ref();
    let power_light_actor_ref = POWER_LIGHT_ACTOR_MAILBOX.actor_ref();
    let status_light_actor_ref = STATUS_LIGHT_ACTOR_MAILBOX.actor_ref();
    let temperature_sensor_actor_ref = TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref();
    let update_actor_ref = UPDATE_ACTOR_MAILBOX.actor_ref();

    let boiler_actor = BoilerActor::new(boiler_actor_ref.clone(), Arc::clone(&config_port), heating_output_port, status_light_actor_ref.clone());
    let brew_actor = BrewActor::new(pump_output_port, valve_output_port);
    let button_actor = ButtonActor::new(button_actor_ref.clone(), boiler_actor_ref.clone(), brew_actor_ref.clone(), power_input_port, status_input_port, status_light_actor_ref.clone(), power_light_actor_ref.clone(), update_actor_ref.clone());
    let water_actor = WaterActor::new(water_actor_ref.clone(), boiler_actor_ref.clone(), brew_actor_ref.clone(), power_light_actor_ref.clone(), water_input_port);
    let power_light_actor = LightActor::new(power_light_output_port, EspTimerPort::new());
    let status_light_actor = LightActor::new(status_light_output_port, EspTimerPort::new());
    let temperature_sensor_actor = TemperatureSensorActor::new(temperature_port, Vec::from_slice(&[boiler_actor_ref.subscriber(&temperature_sensor_actor_ref)]).unwrap());
    let update_actor = UpdateActor::new(update_actor_ref, update_port, button_actor_ref, water_actor_ref, boiler_actor_ref, brew_actor_ref, temperature_sensor_actor_ref, power_light_actor_ref, status_light_actor_ref);

    let actor_group_1 = join3(BOILER_ACTOR_MAILBOX.run(boiler_actor), BREW_ACTOR_MAILBOX.run(brew_actor), BUTTON_ACTOR_MAILBOX.run(button_actor));
    let actor_group_2 = join3(WATER_ACTOR_MAILBOX.run(water_actor), POWER_LIGHT_ACTOR_MAILBOX.run(power_light_actor), STATUS_LIGHT_ACTOR_MAILBOX.run(status_light_actor));
    let actor_group_3 = join3(TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor), UPDATE_ACTOR_MAILBOX.run(update_actor), event_loop());

    join3(actor_group_1, actor_group_2, actor_group_3).await;
}

async fn event_loop() {
    let boiler_actor = BOILER_ACTOR_MAILBOX.actor_ref();
    let button_actor = BUTTON_ACTOR_MAILBOX.actor_ref();
    let water_actor = WATER_ACTOR_MAILBOX.actor_ref();
    let temperature_sensor_actor = TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref();

    loop {
        boiler_actor.tell(None, BoilerMessage::SwitchHeater);
        button_actor.tell(None, ButtonMessage::HandleInputStates);
        water_actor.tell(None, WaterMessage::HandleInputState);
        temperature_sensor_actor.tell(None, TemperatureSensorMessage::MeasureTemperature);

        Timer::after(TICK_INTERVALL).await;
    }
}
