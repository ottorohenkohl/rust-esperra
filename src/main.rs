#![no_std]
#![no_main]

extern crate alloc;
extern crate esp_backtrace;

use crate::adapter::clock::EspTimerAdapter;
use crate::adapter::config::FlashConfigAdapter;
use crate::adapter::gpio::{Ds18b20TemperatureAdapter, EspInputAdapter, EspOutputAdapter};
use crate::adapter::logging::MqttLoggingAdapter;
use crate::adapter::update::EspUpdateAdapter;
use crate::domain::config::actor::ConfigActor;
use crate::domain::config::port::ConfigPort;
use crate::domain::config::protocol::ConfigMessage;
use crate::domain::boiler::actor::BoilerActor;
use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::actor::BrewActor;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::controls::actor::{ButtonActor, WaterActor};
use crate::domain::energy::actor::EnergyActor;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::controls::protocol::{ButtonMessage, WaterMessage};
use crate::domain::gpio::actor::{LightActor, TemperatureSensorActor};
use crate::domain::gpio::protocol::{LightMessage, TemperatureSensorMessage};
use crate::domain::update::actor::UpdateActor;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::implementation::EmbassyMailboxImpl;
use crate::library::messaging::implementation::MqttMessagingImpl;
use crate::library::network::implementation::NetworkStackImpl;

use alloc::sync::Arc;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::Config;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Pull;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::timer::timg::TimerGroup;
use esp_storage::FlashStorage;
use heapless::Vec;
use static_cell::StaticCell;

use embassy_futures::join::{join, join3};

mod adapter;
mod domain;
mod library;

const TICK_INTERVALL: Duration = Duration::from_millis(100);

static BOILER_ACTOR_MAILBOX: EmbassyMailboxImpl<BoilerMessage, 8> = EmbassyMailboxImpl::new();
static BREW_ACTOR_MAILBOX: EmbassyMailboxImpl<BrewMessage, 8> = EmbassyMailboxImpl::new();
static BUTTON_ACTOR_MAILBOX: EmbassyMailboxImpl<ButtonMessage, 8> = EmbassyMailboxImpl::new();
static WATER_ACTOR_MAILBOX: EmbassyMailboxImpl<WaterMessage, 8> = EmbassyMailboxImpl::new();
static STATUS_LIGHT_ACTOR_MAILBOX: EmbassyMailboxImpl<LightMessage, 1> = EmbassyMailboxImpl::new();
static POWER_LIGHT_ACTOR_MAILBOX: EmbassyMailboxImpl<LightMessage, 1> = EmbassyMailboxImpl::new();
static TEMPERATURE_SENSOR_ACTOR_MAILBOX: EmbassyMailboxImpl<TemperatureSensorMessage, 1> = EmbassyMailboxImpl::new();
static UPDATE_ACTOR_MAILBOX: EmbassyMailboxImpl<UpdateMessage, 1> = EmbassyMailboxImpl::new();
static CONFIG_ACTOR_MAILBOX: EmbassyMailboxImpl<ConfigMessage, 4> = EmbassyMailboxImpl::new();
static ENERGY_ACTOR_MAILBOX: EmbassyMailboxImpl<EnergyMessage, 4> = EmbassyMailboxImpl::new();
static NETWORK_STACK: StaticCell<NetworkStackImpl> = StaticCell::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    let mut flash = FlashStorage::new(peripherals.FLASH);

    EspUpdateAdapter::<FlashConfigAdapter>::initialize(&mut flash).unwrap();

    let flash = Arc::new(Mutex::<CriticalSectionRawMutex, _>::new(flash));

    let config_adapter = Arc::new(FlashConfigAdapter::new(Arc::clone(&flash)).await.unwrap());
    let logging_port = Arc::new(MqttLoggingAdapter::new());
    let heating_output_adapter = EspOutputAdapter::new(peripherals.GPIO11);
    let status_light_output_adapter = EspOutputAdapter::new(peripherals.GPIO12);
    let power_light_output_adapter = EspOutputAdapter::new(peripherals.GPIO13);
    let pump_output_adapter = EspOutputAdapter::new(peripherals.GPIO14);
    let temperature_adapter = Ds18b20TemperatureAdapter::new(peripherals.GPIO4).unwrap();
    let valve_output_adapter = EspOutputAdapter::new(peripherals.GPIO10);
    let power_input_adapter = EspInputAdapter::new(peripherals.GPIO20, Pull::Up);
    let status_input_adapter = EspInputAdapter::new(peripherals.GPIO19, Pull::Up);
    let water_input_adapter = EspInputAdapter::new(peripherals.GPIO21, Pull::Up);

    let network = NETWORK_STACK.init(NetworkStackImpl::new(peripherals.WIFI, peripherals.LPWR, peripherals.RNG, peripherals.ADC1, &config_adapter.read_network_info(), spawner));

    let update_adapter = EspUpdateAdapter::new(network, Arc::clone(&flash), Arc::clone(&config_adapter));

    let boiler_actor_ref = BOILER_ACTOR_MAILBOX.actor_ref();
    let brew_actor_ref = BREW_ACTOR_MAILBOX.actor_ref();
    let button_actor_ref = BUTTON_ACTOR_MAILBOX.actor_ref();
    let water_actor_ref = WATER_ACTOR_MAILBOX.actor_ref();
    let power_light_actor_ref = POWER_LIGHT_ACTOR_MAILBOX.actor_ref();
    let status_light_actor_ref = STATUS_LIGHT_ACTOR_MAILBOX.actor_ref();
    let temperature_sensor_actor_ref = TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref();
    let update_actor_ref = UPDATE_ACTOR_MAILBOX.actor_ref();
    let config_actor_ref = CONFIG_ACTOR_MAILBOX.actor_ref();
    let energy_actor_ref = ENERGY_ACTOR_MAILBOX.actor_ref();

    MqttMessagingImpl::new(network, &config_adapter.read_network_info(), boiler_actor_ref.clone(), brew_actor_ref.clone(), update_actor_ref.clone(), config_actor_ref, energy_actor_ref.clone(), spawner);

    let boiler_actor = BoilerActor::new(boiler_actor_ref.clone(), Arc::clone(&config_adapter), heating_output_adapter, status_light_actor_ref.clone());
    let brew_actor = BrewActor::new(pump_output_adapter, valve_output_adapter);
    let button_actor = ButtonActor::new(button_actor_ref.clone(), brew_actor_ref.clone(), power_input_adapter, status_input_adapter, update_actor_ref.clone(), energy_actor_ref.clone());
    let water_actor = WaterActor::new(water_actor_ref.clone(), boiler_actor_ref.clone(), brew_actor_ref.clone(), power_light_actor_ref.clone(), water_input_adapter, Arc::clone(&logging_port));
    let power_light_actor = LightActor::new(power_light_output_adapter, EspTimerAdapter::new());
    let status_light_actor = LightActor::new(status_light_output_adapter, EspTimerAdapter::new());
    let temperature_sensor_actor = TemperatureSensorActor::new(temperature_adapter, Arc::clone(&logging_port), Vec::from_slice(&[boiler_actor_ref.subscriber(&temperature_sensor_actor_ref)]).unwrap());
    let update_actor = UpdateActor::new(update_actor_ref, update_adapter, button_actor_ref, water_actor_ref, boiler_actor_ref.clone(), brew_actor_ref.clone(), temperature_sensor_actor_ref, power_light_actor_ref.clone(), status_light_actor_ref.clone());
    let config_actor = ConfigActor::new(Arc::clone(&config_adapter));
    let energy_actor = EnergyActor::new(energy_actor_ref, boiler_actor_ref.clone(), brew_actor_ref.clone(), power_light_actor_ref.clone(), status_light_actor_ref.clone());

    let actor_group_1 = join3(BOILER_ACTOR_MAILBOX.run(boiler_actor), BREW_ACTOR_MAILBOX.run(brew_actor), BUTTON_ACTOR_MAILBOX.run(button_actor));
    let actor_group_2 = join3(WATER_ACTOR_MAILBOX.run(water_actor), POWER_LIGHT_ACTOR_MAILBOX.run(power_light_actor), STATUS_LIGHT_ACTOR_MAILBOX.run(status_light_actor));
    let actor_group_3 = join3(TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor), UPDATE_ACTOR_MAILBOX.run(update_actor), join(CONFIG_ACTOR_MAILBOX.run(config_actor), join(ENERGY_ACTOR_MAILBOX.run(energy_actor), event_loop())));

    join3(actor_group_1, actor_group_2, actor_group_3).await;
}


async fn event_loop() {
    loop {
        BOILER_ACTOR_MAILBOX.actor_ref().tell(None, BoilerMessage::SwitchHeater);
        BUTTON_ACTOR_MAILBOX.actor_ref().tell(None, ButtonMessage::HandleInputStates);
        WATER_ACTOR_MAILBOX.actor_ref().tell(None, WaterMessage::HandleInputState);
        TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref().tell(None, TemperatureSensorMessage::MeasureTemperature);
        ENERGY_ACTOR_MAILBOX.actor_ref().tell(None, EnergyMessage::CheckInactivity);

        Timer::after(TICK_INTERVALL).await;
    }
}
