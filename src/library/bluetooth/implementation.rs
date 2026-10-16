use super::protocol::{BluetoothCommand, BluetoothConnectionState, BluetoothNotification, BluetoothProfile};

use super::task::bluetooth_task;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use esp_radio::ble::controller::BleConnector;
use trouble_host::prelude::ExternalController;

use esp_hal::peripherals::BT;

pub static COMMANDS: Channel<CriticalSectionRawMutex, BluetoothCommand, 4> = Channel::new();
pub static CONNECTION_RESULT: Signal<CriticalSectionRawMutex, BluetoothConnectionState> = Signal::new();
pub static NOTIFICATIONS: Channel<CriticalSectionRawMutex, BluetoothNotification, 8> = Channel::new();

#[derive(Clone, Copy)]
pub struct BluetoothClient;

impl BluetoothClient {
    pub async fn connect(&self, profile: BluetoothProfile) -> BluetoothConnectionState {
        CONNECTION_RESULT.reset();
        COMMANDS.send(BluetoothCommand::Connect(profile)).await;
        CONNECTION_RESULT.wait().await
    }

    pub async fn write(&self, characteristic_uuid: u16, payload: &[u8]) {
        if let Ok(payload) = heapless::Vec::from_slice(payload) {
            COMMANDS.send(BluetoothCommand::Write(characteristic_uuid, payload)).await;
        }
    }

    pub async fn receive_notification(&self) -> BluetoothNotification {
        NOTIFICATIONS.receive().await
    }
}

pub struct EspBluetooth;

impl EspBluetooth {
    pub fn initialize(bluetooth: BT<'static>, spawner: Spawner) -> BluetoothClient {
        let connector = BleConnector::new(bluetooth, Default::default()).unwrap();
        let controller: ExternalController<_, 1> = ExternalController::new(connector);

        spawner.spawn(bluetooth_task(controller).unwrap());

        BluetoothClient
    }
}
