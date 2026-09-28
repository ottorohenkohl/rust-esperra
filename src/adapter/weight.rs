use crate::domain::weight::port::{ConnectionState, WeightPort};

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{with_timeout, Duration};
use esp_hal::peripherals::BT;
use esp_hal::rng::Rng;
use esp_radio::ble::controller::BleConnector;
use trouble_host::advertise::AdStructure;
use trouble_host::prelude::*;

const BOOKOO_SERVICE_UUID: u16 = 0x0ffe;
const BOOKOO_WEIGHT_UUID: u16 = 0xff11;
const BOOKOO_COMMAND_UUID: u16 = 0xff12;
const WEIGHT_PACKET_LENGTH: usize = 20;
const TARE_COMMAND: [u8; 6] = [0x03, 0x0a, 0x01, 0x00, 0x00, 0x08];
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

enum ScaleCommand {
    Connect,
    Tare,
}

static COMMANDS: Channel<CriticalSectionRawMutex, ScaleCommand, 4> = Channel::new();
static CONNECTION_RESULT: Signal<CriticalSectionRawMutex, ConnectionState> = Signal::new();
static SCALE_ADDRESS: Signal<CriticalSectionRawMutex, (AddrKind, BdAddr)> = Signal::new();
static LATEST_WEIGHT: Mutex<CriticalSectionRawMutex, f64> = Mutex::new(0.0);

pub struct BookooWeightAdapter;

impl BookooWeightAdapter {
    pub fn new(bluetooth: BT<'static>, spawner: Spawner) -> Self {
        let connector = BleConnector::new(bluetooth, Default::default()).unwrap();
        let controller: ExternalController<_, 1> = ExternalController::new(connector);

        spawner.spawn(bookoo_ble_task(controller).unwrap());

        Self
    }
}

impl WeightPort for BookooWeightAdapter {
    async fn connect(&mut self) -> ConnectionState {
        CONNECTION_RESULT.reset();
        COMMANDS.send(ScaleCommand::Connect).await;
        CONNECTION_RESULT.wait().await
    }

    async fn tare(&mut self) {
        COMMANDS.send(ScaleCommand::Tare).await;
    }

    async fn read_weight(&mut self) -> f64 {
        *LATEST_WEIGHT.lock().await
    }
}

struct BookooAdvertisementHandler;

impl EventHandler for BookooAdvertisementHandler {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        while let Some(Ok(report)) = reports.next() {
            if is_bookoo_advertisement(report.data) {
                SCALE_ADDRESS.signal((report.addr_kind, report.addr));
            }
        }
    }
}

fn is_bookoo_advertisement(data: &[u8]) -> bool {
    AdStructure::decode(data).any(|structure| match structure {
        Ok(AdStructure::ServiceUuids16(uuids)) => uuids.iter().any(|uuid| u16::from_le_bytes(*uuid) == BOOKOO_SERVICE_UUID),
        Ok(AdStructure::ServiceData16 { uuid, .. }) => u16::from_le_bytes(uuid) == BOOKOO_SERVICE_UUID,
        Ok(AdStructure::CompleteLocalName(name) | AdStructure::ShortenedLocalName(name)) => name.windows(6).any(|part| part.eq_ignore_ascii_case(b"bookoo")),
        _ => false,
    })
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
async fn bookoo_ble_task(controller: ExternalController<BleConnector<'static>, 1>) {
    let random = Rng::new();
    let mut address_bytes = [
        random.random() as u8,
        random.random() as u8,
        random.random() as u8,
        random.random() as u8,
        random.random() as u8,
        random.random() as u8,
    ];
    address_bytes[5] |= 0xc0;
    let address = Address::random(address_bytes);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host { mut central, mut runner, .. } = stack.build();
    let handler = BookooAdvertisementHandler;

    let _ = join(runner.run_with_handler(&handler), async {
        loop {
            while !matches!(COMMANDS.receive().await, ScaleCommand::Connect) {}

            SCALE_ADDRESS.reset();
            let mut scanner = Scanner::new(central);
            let mut config = ScanConfig::default();
            config.active = true;
            config.interval = Duration::from_millis(100);
            config.window = Duration::from_millis(100);

            let scan_result = match scanner.scan(&config).await {
                Ok(session) => {
                    let result = with_timeout(SCAN_TIMEOUT, SCALE_ADDRESS.wait()).await;
                    drop(session);
                    result.ok()
                },
                Err(_) => None,
            };
            central = scanner.into_inner();

            let Some((address_kind, address)) = scan_result else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };

            let accepted = [(address_kind, &address)];
            let connect_config = ConnectConfig {
                scan_config: ScanConfig { filter_accept_list: &accepted, ..Default::default() },
                connect_params: Default::default(),
            };

            let Ok(connection) = central.connect(&connect_config).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };

            let Ok(client) = GattClient::<_, DefaultPacketPool, 4>::new(&stack, &connection).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };

            let Ok(services) = client.services_by_uuid(&Uuid::new_short(BOOKOO_SERVICE_UUID)).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };
            let Some(service) = services.first() else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };

            let Ok(weight_characteristic) = client.characteristic_by_uuid::<[u8]>(service, &Uuid::new_short(BOOKOO_WEIGHT_UUID)).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };
            let Ok(command_characteristic) = client.characteristic_by_uuid::<[u8]>(service, &Uuid::new_short(BOOKOO_COMMAND_UUID)).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };
            let Ok(mut notifications) = client.subscribe(&weight_characteristic, false).await else {
                CONNECTION_RESULT.signal(ConnectionState::ConnectionFailed);
                continue
            };

            *LATEST_WEIGHT.lock().await = 0.0;
            CONNECTION_RESULT.signal(ConnectionState::ConnectionEstablished);

            let session = async {
                loop {
                    match select(notifications.next(), COMMANDS.receive()).await {
                        Either::First(notification) => {
                            if let Some(weight) = parse_weight_packet(notification.as_ref()) {
                                *LATEST_WEIGHT.lock().await = weight;
                            }
                        },
                        Either::Second(ScaleCommand::Connect) => CONNECTION_RESULT.signal(ConnectionState::AlreadyConnected),
                        Either::Second(ScaleCommand::Tare) => {
                            let _ = client.write_characteristic(&command_characteristic, &TARE_COMMAND).await;
                        },
                    }
                }
            };

            let _ = select(client.task(), session).await;
        }
    }).await;
}
