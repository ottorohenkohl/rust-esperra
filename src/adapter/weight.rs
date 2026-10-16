
use crate::domain::weight::port::{ConnectionState, WeightPort};
use crate::library::bluetooth::implementation::BluetoothClient;
use crate::library::bluetooth::protocol::{BluetoothConnectionState, BluetoothProfile};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

const BOOKOO_AUTOMATIC_MODE_PACKET_TYPE: u8 = 0x0d;
const BOOKOO_EVENT_UUID: u16 = 0xff12;
const BOOKOO_SHUTDOWN_COMMAND: [u8; 6] = [0x03, 0x0a, 0x15, 0x00, 0x00, 0x1c];
const BOOKOO_PACKET_LENGTH: usize = 20;
const BOOKOO_READY_EVENT: u8 = 0x02;
const BOOKOO_SERVICE_UUID: u16 = 0x0ffe;
const BOOKOO_WEIGHT_UUID: u16 = 0xff11;
const BOOKOO_WEIGHT_PACKET_TYPE: u8 = 0x0b;

const BOOKOO_ADVERTISEMENT_NAME: &str = "bookoo";

const BOOKOO_PROFILE: BluetoothProfile = BluetoothProfile {
    advertisement_service_uuid: BOOKOO_SERVICE_UUID,
    advertisement_name_prefix: BOOKOO_ADVERTISEMENT_NAME,
    service_uuid: BOOKOO_SERVICE_UUID,
    notification_uuids: [BOOKOO_WEIGHT_UUID, BOOKOO_EVENT_UUID],
};

static LATEST_WEIGHT: Mutex<CriticalSectionRawMutex, f64> = Mutex::new(0.0);
static READY_STATE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

pub struct BookooWeightAdapter {
    bluetooth: BluetoothClient,
}

impl BookooWeightAdapter {
    pub fn new(bluetooth: BluetoothClient, spawner: Spawner) -> Self {
        spawner.spawn(bookoo_notification_task(bluetooth).unwrap());

        Self { bluetooth }
    }
}

impl WeightPort for BookooWeightAdapter {
    async fn connect(&mut self) -> ConnectionState {
        match self.bluetooth.connect(BOOKOO_PROFILE).await {
            BluetoothConnectionState::AlreadyConnected => ConnectionState::AlreadyConnected,
            BluetoothConnectionState::ConnectionEstablished => {
                *LATEST_WEIGHT.lock().await = 0.0;
                *READY_STATE.lock().await = false;

                ConnectionState::ConnectionEstablished
            },
            BluetoothConnectionState::ConnectionFailed => ConnectionState::ConnectionFailed,
        }
    }

    async fn shutdown(&mut self) {
        self.bluetooth.write(BOOKOO_EVENT_UUID, &BOOKOO_SHUTDOWN_COMMAND).await;
    }

    async fn reset_ready_state(&mut self) {
        *READY_STATE.lock().await = false;
    }

    async fn is_ready_state(&mut self) -> bool {
        *READY_STATE.lock().await
    }

    async fn read_weight(&mut self) -> f64 {
        *LATEST_WEIGHT.lock().await
    }
}

fn is_valid_packet(packet: &[u8], packet_type: u8) -> bool {
    if packet.len() != BOOKOO_PACKET_LENGTH {
        return false
    }

    let has_valid_product_number = packet[0] == 0x03;
    let has_expected_packet_type = packet[1] == packet_type;

    let calculated_checksum = packet[..BOOKOO_PACKET_LENGTH - 1].iter().copied().fold(0, |checksum, byte| checksum ^ byte);
    let received_checksum = packet[BOOKOO_PACKET_LENGTH - 1];

    let has_valid_checksum = calculated_checksum == received_checksum;

    has_valid_product_number && has_expected_packet_type && has_valid_checksum
}

fn parse_weight_packet(packet: &[u8]) -> Option<f64> {
    if !is_valid_packet(packet, BOOKOO_WEIGHT_PACKET_TYPE) {
        return None
    }

    let magnitude = u32::from_be_bytes([0, packet[7], packet[8], packet[9]]) as f64 / 100.0;

    let sign = match packet[6] {
        0x00 | b'+' => 1.0,
        0x01 | b'-' => -1.0,

        _ => return None,
    };

    Some(sign * magnitude)
}

fn is_ready_event(packet: &[u8]) -> bool {
    is_valid_packet(packet, BOOKOO_AUTOMATIC_MODE_PACKET_TYPE) && packet[2] == BOOKOO_READY_EVENT
}

#[embassy_executor::task]
async fn bookoo_notification_task(bluetooth: BluetoothClient) {
    loop {
        let notification = bluetooth.receive_notification().await;

        let packet = notification.payload.as_slice();

        match notification.characteristic_uuid {
            BOOKOO_WEIGHT_UUID => {
                if let Some(weight) = parse_weight_packet(packet) {
                    *LATEST_WEIGHT.lock().await = weight;
                }
            },
            BOOKOO_EVENT_UUID => {
                if is_ready_event(packet) {
                    *READY_STATE.lock().await = true;
                }
            },
            _ => {},
        }
    }
}
