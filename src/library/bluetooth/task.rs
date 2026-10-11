use super::protocol::{BluetoothCommand, BluetoothConnectionState, BluetoothNotification, BluetoothProfile};

use super::implementation::{COMMANDS, CONNECTION_RESULT, NOTIFICATIONS};

use core::cell::RefCell;
use embassy_futures::select::Either3;
use embassy_time::{with_timeout, Duration};
use esp_hal::rng::Rng;
use esp_radio::ble::controller::BleConnector;
use heapless::Vec;
use trouble_host::advertise::AdStructure;
use trouble_host::prelude::*;

use critical_section::Mutex as CriticalSectionMutex;

use embassy_futures::select::{select, select3};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const GATT_TIMEOUT: Duration = Duration::from_secs(10);
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

struct AdvertisementHandler {
    profile: CriticalSectionMutex<RefCell<Option<BluetoothProfile>>>,
}

impl AdvertisementHandler {
    const fn new() -> Self {
        let profile = CriticalSectionMutex::new(RefCell::new(None));

        Self { profile }
    }

    fn set_profile(&self, profile: BluetoothProfile) {
        critical_section::with(|cs| *self.profile.borrow_ref_mut(cs) = Some(profile));
    }
}

impl EventHandler for AdvertisementHandler {
    fn on_adv_reports(&self, mut reports: LeAdvReportsIter<'_>) {
        let profile = critical_section::with(|cs| *self.profile.borrow_ref(cs));

        let Some(profile) = profile else { return };

        while let Some(Ok(report)) = reports.next() {
            if matches_advertisement(report.data, profile) {
                SCALE_ADDRESS.signal((report.addr_kind, report.addr));
            }
        }
    }
}

static SCALE_ADDRESS: embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, (AddrKind, BdAddr)> = embassy_sync::signal::Signal::new();

fn matches_advertisement(data: &[u8], profile: BluetoothProfile) -> bool {
    AdStructure::decode(data).any(|structure| match structure {
        Ok(AdStructure::ServiceUuids16(uuids)) => uuids.iter().any(|uuid| u16::from_le_bytes(*uuid) == profile.advertisement_service_uuid),
        Ok(AdStructure::ServiceData16 { uuid, .. }) => u16::from_le_bytes(uuid) == profile.advertisement_service_uuid,
        Ok(AdStructure::CompleteLocalName(name) | AdStructure::ShortenedLocalName(name)) => {
            let prefix = profile.advertisement_name_prefix.as_bytes();

            name.get(..prefix.len()).is_some_and(|name| name.eq_ignore_ascii_case(prefix))
        },
        _ => false,
    })
}

#[embassy_executor::task]
pub async fn bluetooth_task(controller: ExternalController<BleConnector<'static>, 1>) {
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

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> = HostResources::new();

    let stack = trouble_host::new(controller, &mut resources).set_random_address(Address::random(address_bytes));

    let Host { mut central, mut runner, .. } = stack.build();

    let handler = AdvertisementHandler::new();

    let _ = select(runner.run_with_handler(&handler), async {
        loop {
            let profile = loop {
                match COMMANDS.receive().await {
                    BluetoothCommand::Connect(profile) => break profile,
                }
            };

            handler.set_profile(profile);
            SCALE_ADDRESS.reset();

            let mut scanner = Scanner::new(central);
            let mut scan_config = ScanConfig::default();

            scan_config.active = true;
            scan_config.interval = Duration::from_millis(100);
            scan_config.window = Duration::from_millis(100);

            let scan_result = match scanner.scan(&scan_config).await {
                Ok(session) => {
                    let result = with_timeout(SCAN_TIMEOUT, SCALE_ADDRESS.wait()).await;

                    drop(session);

                    result.ok()
                },
                Err(_) => None,
            };
            central = scanner.into_inner();

            let Some((address_kind, address)) = scan_result else {
                CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed);

                continue
            };

            let accepted = [(address_kind, &address)];

            let connect_config = ConnectConfig {
                scan_config: ScanConfig { filter_accept_list: &accepted, ..Default::default() },
                connect_params: Default::default(),
            };

            let Ok(Ok(connection)) = with_timeout(CONNECTION_TIMEOUT, central.connect(&connect_config)).await else {
                CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed);

                continue
            };

            let Ok(Ok(client)) = with_timeout(GATT_TIMEOUT, GattClient::<_, DefaultPacketPool, 4>::new(&stack, &connection)).await else {
                CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed);

                continue
            };

            let session = async {
                let discovery = async {
                    let services = client.services_by_uuid(&Uuid::new_short(profile.service_uuid)).await.ok()?;
                    let service = services.first()?;
                    let first_characteristic = client.characteristic_by_uuid::<[u8]>(service, &Uuid::new_short(profile.notification_uuids[0])).await.ok()?;
                    let second_characteristic = client.characteristic_by_uuid::<[u8]>(service, &Uuid::new_short(profile.notification_uuids[1])).await.ok()?;
                    let first_notifications = client.subscribe(&first_characteristic, false).await.ok()?;
                    let second_notifications = client.subscribe(&second_characteristic, false).await.ok()?;

                    Some((first_notifications, second_notifications))
                };

                let Ok(Some((mut first_notifications, mut second_notifications))) = with_timeout(GATT_TIMEOUT, discovery).await else {
                    CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed);

                    return
                };

                CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionEstablished);

                loop {
                    let (characteristic_uuid, notification) = match select3(first_notifications.next(), second_notifications.next(), COMMANDS.receive()).await {
                        Either3::First(notification) => (profile.notification_uuids[0], notification),
                        Either3::Second(notification) => (profile.notification_uuids[1], notification),
                        Either3::Third(BluetoothCommand::Connect(_)) => {
                            CONNECTION_RESULT.signal(BluetoothConnectionState::AlreadyConnected);

                            continue
                        },
                    };

                    if let Ok(payload) = Vec::<u8, 64>::from_slice(notification.as_ref()) {
                        let _ = NOTIFICATIONS.try_send(BluetoothNotification { characteristic_uuid, payload });
                    }
                }
            };

            let disconnected = async {
                loop {
                    if matches!(connection.next().await, ConnectionEvent::Disconnected { .. }) {
                        return
                    }
                }
            };

            let _ = select3(client.task(), session, disconnected).await;

            CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed);
        }
    }).await;

    loop {
        match COMMANDS.receive().await {
            BluetoothCommand::Connect(_) => CONNECTION_RESULT.signal(BluetoothConnectionState::ConnectionFailed),
        }
    }
}
