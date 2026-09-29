#![no_std]
#![no_main]

extern crate alloc;
extern crate esp_backtrace;

use crate::adapter::clock::EspTimerAdapter;
use crate::adapter::config::FlashConfigAdapter;
use crate::adapter::gpio::{Ds18b20TemperatureAdapter, EspInputAdapter, EspOutputAdapter};
use crate::adapter::logging::MqttLoggingAdapter;
use crate::adapter::update::EspUpdateAdapter;
use crate::adapter::weight::BookooWeightAdapter;
use crate::domain::config::actor::ConfigActor;
use crate::domain::config::port::ReadConfigPort;
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
use crate::domain::weight::actor::WeightActor;
use crate::domain::weight::protocol::WeightMessage;
use crate::library::actor::implementation::EmbassyMailboxImpl;
use crate::library::bluetooth::implementation::EspBluetooth;
use crate::library::debug::implementation::DebugImpl;
use crate::library::debug::protocol::Color;
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

const FAST_TICK_INTERVAL: Duration = Duration::from_millis(100);
const SLOW_TICK_INTERVAL: Duration = Duration::from_millis(1000);

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
static WEIGHT_ACTOR_MAILBOX: EmbassyMailboxImpl<WeightMessage, 4> = EmbassyMailboxImpl::new();
static NETWORK_STACK: StaticCell<NetworkStackImpl> = StaticCell::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 96 * 1024);

    let peripherals = esp_hal::init(Config::default().with_cpu_clock(CpuClock::max()));

    esp_rtos::start(TimerGroup::new(peripherals.TIMG0).timer0, SoftwareInterruptControl::new(peripherals.SW_INTERRUPT).software_interrupt0);

    DebugImpl::initialize(peripherals.RMT, peripherals.GPIO48, peripherals.GPIO38, spawner);
    DebugImpl::set(Color::Red).await;

    let mut flash = FlashStorage::new(peripherals.FLASH);

    EspUpdateAdapter::<FlashConfigAdapter>::initialize(&mut flash).unwrap();

    let flash = Arc::new(Mutex::<CriticalSectionRawMutex, _>::new(flash));
    let config_adapter = Arc::new(FlashConfigAdapter::new(Arc::clone(&flash)).await.unwrap());

    DebugImpl::set(Color::White).await;

    let heating_output_adapter = EspOutputAdapter::new(peripherals.GPIO21, false);
    let status_light_output_adapter = EspOutputAdapter::new(peripherals.GPIO39, true);
    let power_light_output_adapter = EspOutputAdapter::new(peripherals.GPIO40, true);
    let pump_output_adapter = EspOutputAdapter::new(peripherals.GPIO41, true);
    let valve_output_adapter = EspOutputAdapter::new(peripherals.GPIO42, true);

    DebugImpl::set(Color::Magenta).await;

    let bluetooth = EspBluetooth::initialize(peripherals.BT, spawner);

    DebugImpl::set(Color::Orange).await;

    let weight_adapter = BookooWeightAdapter::new(bluetooth, spawner);
    let temperature_adapter = Ds18b20TemperatureAdapter::new(peripherals.GPIO15);

    DebugImpl::set(Color::Yellow).await;

    let power_input_adapter = EspInputAdapter::new(peripherals.GPIO18, Pull::Up);
    let status_input_adapter = EspInputAdapter::new(peripherals.GPIO17, Pull::Up);
    let water_input_adapter = EspInputAdapter::new(peripherals.GPIO16, Pull::Up);

    let network = NETWORK_STACK.init(NetworkStackImpl::new(peripherals.WIFI, peripherals.LPWR, peripherals.RNG, peripherals.ADC1, &config_adapter.read_network_info(), spawner));

    DebugImpl::set(Color::Green).await;

    let logging_adapter = Arc::new(MqttLoggingAdapter::new());
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
    let weight_actor_ref = WEIGHT_ACTOR_MAILBOX.actor_ref();

    let boiler_actor = BoilerActor::new("boiler", boiler_actor_ref.clone(), Arc::clone(&config_adapter), heating_output_adapter, status_light_actor_ref.clone());
    let brew_actor = BrewActor::new("brew", pump_output_adapter, valve_output_adapter);
    let button_actor = ButtonActor::new("button", button_actor_ref.clone(), brew_actor_ref.clone(), power_input_adapter, status_input_adapter, update_actor_ref.clone(), energy_actor_ref.clone());
    let water_actor = WaterActor::new("water", water_actor_ref.clone(), boiler_actor_ref.clone(), brew_actor_ref.clone(), power_light_actor_ref.clone(), water_input_adapter, Arc::clone(&logging_adapter));
    let power_light_actor = LightActor::new("light/power", power_light_output_adapter, EspTimerAdapter::new(), Arc::clone(&logging_adapter));
    let status_light_actor = LightActor::new("light/status", status_light_output_adapter, EspTimerAdapter::new(), Arc::clone(&logging_adapter));
    let temperature_sensor_actor = TemperatureSensorActor::new("temperature", temperature_adapter, Arc::clone(&logging_adapter), Vec::from_slice(&[boiler_actor_ref.subscriber(&temperature_sensor_actor_ref)]).unwrap());
    let update_actor = UpdateActor::new("update", update_actor_ref.clone(), update_adapter, button_actor_ref, water_actor_ref, boiler_actor_ref.clone(), brew_actor_ref.clone(), temperature_sensor_actor_ref, power_light_actor_ref.clone(), status_light_actor_ref.clone());
    let config_actor = ConfigActor::new("config", Arc::clone(&config_adapter), Arc::clone(&logging_adapter));
    let energy_actor = EnergyActor::new("energy", energy_actor_ref.clone(), boiler_actor_ref.clone(), brew_actor_ref.clone(), power_light_actor_ref.clone(), status_light_actor_ref.clone(), Arc::clone(&logging_adapter));
    let weight_actor = WeightActor::new("weight", weight_adapter, Arc::clone(&logging_adapter), energy_actor_ref.clone(), brew_actor_ref.clone(), Arc::clone(&config_adapter));

    DebugImpl::set(Color::Cyan).await;

    let mqtt_config_adapter: Arc<dyn ReadConfigPort> = config_adapter.clone();

    MqttMessagingImpl::new(network, mqtt_config_adapter, boiler_actor_ref.clone(), brew_actor_ref.clone(), update_actor_ref.clone(), config_actor_ref, energy_actor_ref.clone(), weight_actor_ref, spawner);

    DebugImpl::set(Color::Blue).await;

    let actor_group_1 = join3(BOILER_ACTOR_MAILBOX.run(boiler_actor), BREW_ACTOR_MAILBOX.run(brew_actor), BUTTON_ACTOR_MAILBOX.run(button_actor));
    let actor_group_2 = join3(WATER_ACTOR_MAILBOX.run(water_actor), POWER_LIGHT_ACTOR_MAILBOX.run(power_light_actor), STATUS_LIGHT_ACTOR_MAILBOX.run(status_light_actor));
    let actor_group_3 = join3(TEMPERATURE_SENSOR_ACTOR_MAILBOX.run(temperature_sensor_actor), UPDATE_ACTOR_MAILBOX.run(update_actor), WEIGHT_ACTOR_MAILBOX.run(weight_actor));
    let actor_group_4 = join(CONFIG_ACTOR_MAILBOX.run(config_actor), ENERGY_ACTOR_MAILBOX.run(energy_actor));

    let event_loops = join(fast_event_loop(), slow_event_loop());

    join3(join(actor_group_1, actor_group_2), join(actor_group_3, actor_group_4), event_loops).await;
}

async fn fast_event_loop() {
    loop {
        BOILER_ACTOR_MAILBOX.actor_ref().system_tell(BoilerMessage::SwitchHeater);

        BUTTON_ACTOR_MAILBOX.actor_ref().tell(None, ButtonMessage::HandleInputStates);
        WATER_ACTOR_MAILBOX.actor_ref().tell(None, WaterMessage::HandleInputState);

        Timer::after(FAST_TICK_INTERVAL).await;
    }
}

async fn slow_event_loop() {
    loop {
        TEMPERATURE_SENSOR_ACTOR_MAILBOX.actor_ref().tell(None, TemperatureSensorMessage::MeasureTemperature);
        ENERGY_ACTOR_MAILBOX.actor_ref().tell(None, EnergyMessage::CheckInactivity);
        CONFIG_ACTOR_MAILBOX.actor_ref().tell(None, ConfigMessage::LogTemperatureOptions);
        CONFIG_ACTOR_MAILBOX.actor_ref().tell(None, ConfigMessage::LogFirmwareInfo);
        CONFIG_ACTOR_MAILBOX.actor_ref().tell(None, ConfigMessage::LogPidGains);
        ENERGY_ACTOR_MAILBOX.actor_ref().tell(None, EnergyMessage::LogActiveState);
        WEIGHT_ACTOR_MAILBOX.actor_ref().tell(None, WeightMessage::MonitorWeight);

        Timer::after(SLOW_TICK_INTERVAL).await;
    }
}
