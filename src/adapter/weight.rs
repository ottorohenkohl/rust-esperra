use crate::domain::weight::port::{ConnectionState, WeightPort};
use crate::library::bluetooth::implementation::BluetoothClient;
use crate::library::bluetooth::protocol::{BluetoothConnectionState, BluetoothProfile};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

const BOOKOO_COMMAND_UUID: u16 = 0xff12;
const BOOKOO_SERVICE_UUID: u16 = 0x0ffe;
const BOOKOO_WEIGHT_UUID: u16 = 0xff11;
const TARE_COMMAND: [u8; 6] = [0x03, 0x0a, 0x01, 0x00, 0x00, 0x08];
const WEIGHT_PACKET_LENGTH: usize = 20;

const BOOKOO_PROFILE: BluetoothProfile = BluetoothProfile {
    advertisement_service_uuid: BOOKOO_SERVICE_UUID,
    advertisement_name_prefix: "bookoo",
    service_uuid: BOOKOO_SERVICE_UUID,
    notification_uuid: BOOKOO_WEIGHT_UUID,
    write_uuid: BOOKOO_COMMAND_UUID,
};

static LATEST_WEIGHT: Mutex<CriticalSectionRawMutex, f64> = Mutex::new(0.0);

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
                ConnectionState::ConnectionEstablished
            },
            BluetoothConnectionState::ConnectionFailed => ConnectionState::ConnectionFailed,
        }
    }

    async fn tare(&mut self) {
        let _ = self.bluetooth.write(BOOKOO_COMMAND_UUID, &TARE_COMMAND).await;
    }

    async fn read_weight(&mut self) -> f64 {
        *LATEST_WEIGHT.lock().await
    }
}

fn parse_weight_packet(packet: &[u8]) -> Option<f64> {
    if packet.len() != WEIGHT_PACKET_LENGTH || packet[0] != 0x03 || packet[1] != 0x0b {
        return None
    }

    if packet[..WEIGHT_PACKET_LENGTH - 1].iter().copied().fold(0, |checksum, byte| checksum ^ byte) != packet[WEIGHT_PACKET_LENGTH - 1] {
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

#[embassy_executor::task]
async fn bookoo_notification_task(bluetooth: BluetoothClient) {
    loop {
        let notification = bluetooth.receive_notification().await;

        if notification.characteristic_uuid == BOOKOO_WEIGHT_UUID
            && let Some(weight) = parse_weight_packet(notification.payload.as_slice())
        {
            *LATEST_WEIGHT.lock().await = weight;
        }
    }
}
