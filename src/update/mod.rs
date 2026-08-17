mod manifest;

use embassy_net::{Runner, Stack, StackResources, dns::DnsSocket, tcp::client::{TcpClient, TcpClientState}};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use embedded_storage::Storage;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_hal::rng::Rng;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, sta::StationConfig};
use esp_storage::FlashStorage;
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::request::{Method, RequestBuilder};
use sha2::{Digest, Sha256};
use static_cell::StaticCell;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::library::actor::reference::ActorRef;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
const MANIFEST_URL: &str = match option_env!("ESPERRA_UPDATE_MANIFEST_URL") { Some(value) => value, None => "" };
const WIFI_SSID: &str = match option_env!("ESPERRA_WIFI_SSID") { Some(value) => value, None => "" };
const WIFI_PASSWORD: &str = match option_env!("ESPERRA_WIFI_PASSWORD") { Some(value) => value, None => "" };
const GITLAB_TOKEN: &str = match option_env!("ESPERRA_GITLAB_TOKEN") { Some(value) => value, None => "" };
const UPDATE_PUBLIC_KEY: [u8; 32] = include!(concat!(env!("OUT_DIR"), "/update_public_key.rs"));
const UPDATE_HEADERS: [(&str, &str); 2] = [("Connection", "close"), ("PRIVATE-TOKEN", GITLAB_TOKEN)];

static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();

#[derive(Debug)]
enum Error {
    Flash,
    Http,
    InvalidImage,
    Manifest,
}

pub fn initialize(flash: &mut FlashStorage<'_>) {
    if UPDATE_PUBLIC_KEY == [0; 32] {
        log::error!("OTA public key is not configured");
        return;
    }

    let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    if OtaUpdater::new(flash, &mut table).is_err() {
        log::error!("invalid OTA partition table");
    }
}

pub fn start(wifi: esp_hal::peripherals::WIFI<'static>, flash: FlashStorage<'static>, boiler: ActorRef<BoilerMessage>, brewing: ActorRef<BrewMessage>, spawner: embassy_executor::Spawner) {
    if UPDATE_PUBLIC_KEY == [0; 32] || MANIFEST_URL.is_empty() || WIFI_SSID.is_empty() {
        return;
    }

    let station = Config::Station(StationConfig::default().with_ssid(WIFI_SSID).with_password(WIFI_PASSWORD.into()));
    let (controller, interfaces) = esp_radio::wifi::new(wifi, ControllerConfig::default().with_initial_config(station)).unwrap();
    let interface = interfaces.station;
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let (stack, runner) = embassy_net::new(interface, embassy_net::Config::dhcpv4(Default::default()), STACK_RESOURCES.init(StackResources::new()), seed);

    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(network_task(runner).unwrap());
    spawner.spawn(update_task(stack, flash, boiler, brewing, seed).unwrap());
}

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        if controller.connect_async().await.is_ok() {
            let _ = controller.wait_for_disconnect_async().await;
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await;
}

#[embassy_executor::task]
async fn update_task(stack: Stack<'static>, mut flash: FlashStorage<'static>, boiler: ActorRef<BoilerMessage>, brewing: ActorRef<BrewMessage>, seed: u64) {
    Timer::after(Duration::from_secs(10)).await;
    confirm_running_image(&mut flash);
    stack.wait_config_up().await;
    loop {
        match check_and_install(stack, &mut flash, &boiler, &brewing, seed).await {
            Ok(true) => esp_hal::system::software_reset(),
            Ok(false) => Timer::after(CHECK_INTERVAL).await,
            Err(error) => {
                log::warn!("update check failed: {:?}", error);
                Timer::after(RETRY_INTERVAL).await;
            }
        }
    }
}

fn confirm_running_image(flash: &mut FlashStorage<'_>) {
    let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    let Ok(mut updater) = OtaUpdater::new(flash, &mut table) else { return };
    if matches!(updater.current_ota_state(), Ok(OtaImageState::New | OtaImageState::PendingVerify)) && updater.set_current_ota_state(OtaImageState::Valid).is_ok() {
        log::info!("confirmed OTA image {}", env!("CARGO_PKG_VERSION"));
    }
}

async fn check_and_install(stack: Stack<'static>, flash: &mut FlashStorage<'static>, boiler: &ActorRef<BoilerMessage>, brewing: &ActorRef<BrewMessage>, seed: u64) -> Result<bool, Error> {
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    let mut tls_rx = [0; 16384];
    let mut tls_tx = [0; 4096];
    let tls = TlsConfig::new(seed, &mut tls_rx, &mut tls_tx, TlsVerify::None);
    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls);
    let mut response_buffer = [0; 2048];
    let request = client.request(Method::GET, MANIFEST_URL).await.map_err(|_| Error::Http)?;
    let mut request = request.headers(&UPDATE_HEADERS);
    let response = request.send(&mut response_buffer).await.map_err(|_| Error::Http)?;
    if !response.status.is_successful() {
        return Err(Error::Http);
    }
    let body = response.body().read_to_end().await.map_err(|_| Error::Http)?;
    let manifest = manifest::parse(body).map_err(|_| Error::Manifest)?;
    manifest.verify(&UPDATE_PUBLIC_KEY).map_err(|_| Error::Manifest)?;
    drop(request);
    if !manifest.is_newer_than(env!("CARGO_PKG_VERSION")) {
        return Ok(false);
    }

    boiler.tell(BoilerMessage::SafetyShutdown);
    brewing.tell(BrewMessage::SetBrewing(false));
    Timer::after(Duration::from_secs(2)).await;

    let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    let mut updater = OtaUpdater::new(flash, &mut table).map_err(|_| Error::Flash)?;
    let (mut partition, _) = updater.next_partition().map_err(|_| Error::Flash)?;
    if manifest.size == 0 || manifest.size > partition.partition_size() {
        return Err(Error::InvalidImage);
    }

    let mut firmware_buffer = [0; 4096];
    let request = client.request(Method::GET, &manifest.url).await.map_err(|_| Error::Http)?;
    let mut request = request.headers(&UPDATE_HEADERS);
    let response = request.send(&mut firmware_buffer).await.map_err(|_| Error::Http)?;
    if !response.status.is_successful() || response.content_length != Some(manifest.size) {
        return Err(Error::Http);
    }

    let mut reader = response.body().reader();
    let mut chunk = [0; 4096];
    let mut offset = 0;
    let mut hash = Sha256::new();
    loop {
        let read = reader.read(&mut chunk).await.map_err(|_| Error::Http)?;
        if read == 0 {
            break;
        }
        if offset + read > manifest.size {
            return Err(Error::InvalidImage);
        }
        partition.write(offset as u32, &chunk[..read]).map_err(|_| Error::Flash)?;
        hash.update(&chunk[..read]);
        offset += read;
    }

    if offset != manifest.size || hash.finalize().as_slice() != manifest.expected_hash().map_err(|_| Error::Manifest)? {
        return Err(Error::InvalidImage);
    }
    drop(partition);
    updater.activate_next_partition().map_err(|_| Error::Flash)?;
    updater.set_current_ota_state(OtaImageState::New).map_err(|_| Error::Flash)?;
    Ok(true)
}
