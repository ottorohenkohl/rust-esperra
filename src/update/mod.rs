mod manifest;

use core::ffi::CStr;
use core::net::{IpAddr, SocketAddr};

use embassy_net::{
    Runner, Stack, StackResources,
    dns::{DnsQueryType, DnsSocket},
    tcp::client::{TcpClient, TcpClientState},
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use embedded_storage::Storage;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_hal::rng::{Rng, Trng, TrngSource};
use esp_hal::rtc_cntl::Rtc;
use esp_radio::wifi::{Config, ControllerConfig, Interface, WifiController, sta::StationConfig};
use esp_storage::FlashStorage;
use mbedtls_rs::sys::hook::backend::embassy::timer::EmbassyTimer;
use mbedtls_rs::sys::hook::backend::esp::wall_clock::EspRtcWallClock;
use mbedtls_rs::Tls;
use reqwless::client::{HttpClient, TlsConfig};
use reqwless::request::{Method, RequestBuilder};
use reqwless::{Certificate, TlsVersion, X509};
use sha2::{Digest, Sha256};
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use static_cell::StaticCell;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::library::actor::reference::ActorRef;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
const NTP_SERVER: &str = "pool.ntp.org";
const MANIFEST_URL: &str = match option_env!("ESPERRA_UPDATE_MANIFEST_URL") { Some(value) => value, None => "" };
const WIFI_SSID: &str = match option_env!("ESPERRA_WIFI_SSID") { Some(value) => value, None => "" };
const WIFI_PASSWORD: &str = match option_env!("ESPERRA_WIFI_PASSWORD") { Some(value) => value, None => "" };
const GITLAB_TOKEN: &str = match option_env!("ESPERRA_GITLAB_TOKEN") { Some(value) => value, None => "" };
const UPDATE_HEADERS: [(&str, &str); 2] = [("Connection", "close"), ("PRIVATE-TOKEN", GITLAB_TOKEN)];
const ROOT_CA: &CStr = match CStr::from_bytes_with_nul(concat!(include_str!("isrg-root-x1.pem"), "\0").as_bytes()) {
    Ok(certificate) => certificate,
    Err(_) => panic!("invalid OTA root CA"),
};

static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();
static RTC: StaticCell<Rtc<'static>> = StaticCell::new();
static TRNG_SOURCE: StaticCell<TrngSource<'static>> = StaticCell::new();
static TRNG: StaticCell<Trng> = StaticCell::new();
static TLS_TIMER: StaticCell<EmbassyTimer> = StaticCell::new();
static TLS_CLOCK: StaticCell<EspRtcWallClock<&'static Rtc<'static>>> = StaticCell::new();

#[derive(Debug)]
enum Error {
    Flash,
    Http,
    InvalidImage,
    Manifest,
    Time,
}

#[derive(Clone, Copy)]
struct Timestamp {
    rtc: &'static Rtc<'static>,
    current_time_us: u64,
}

impl NtpTimestampGenerator for Timestamp {
    fn init(&mut self) {
        self.current_time_us = self.rtc.current_time_us();
    }

    fn timestamp_sec(&self) -> u64 {
        self.current_time_us / 1_000_000
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.current_time_us % 1_000_000) as u32
    }
}

pub fn initialize(flash: &mut FlashStorage<'_>) {
    let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    if OtaUpdater::new(flash, &mut table).is_err() {
        log::error!("invalid OTA partition table");
    }
}

pub fn start(
    wifi: esp_hal::peripherals::WIFI<'static>,
    lpwr: esp_hal::peripherals::LPWR<'static>,
    rng: esp_hal::peripherals::RNG<'static>,
    adc1: esp_hal::peripherals::ADC1<'static>,
    flash: FlashStorage<'static>,
    boiler: ActorRef<BoilerMessage>,
    brewing: ActorRef<BrewMessage>,
    spawner: embassy_executor::Spawner,
) {
    if MANIFEST_URL.is_empty() || WIFI_SSID.is_empty() {
        return;
    }

    let station = Config::Station(StationConfig::default().with_ssid(WIFI_SSID).with_password(WIFI_PASSWORD.into()));
    let (controller, interfaces) = esp_radio::wifi::new(wifi, ControllerConfig::default().with_initial_config(station)).unwrap();
    let random = Rng::new();
    let seed = (random.random() as u64) << 32 | random.random() as u64;
    let (stack, runner) = embassy_net::new(interfaces.station, embassy_net::Config::dhcpv4(Default::default()), STACK_RESOURCES.init(StackResources::new()), seed);

    let rtc = RTC.init(Rtc::new(lpwr));
    TRNG_SOURCE.init(TrngSource::new(rng, adc1));
    let trng = TRNG.init(Trng::try_new().unwrap());
    unsafe {
        mbedtls_rs::sys::hook::timer::hook_timer(Some(TLS_TIMER.init(EmbassyTimer)));
        mbedtls_rs::sys::hook::wall_clock::hook_wall_clock(Some(TLS_CLOCK.init(EspRtcWallClock::new(rtc))));
    }
    let tls = Tls::new(trng).unwrap();

    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(network_task(runner).unwrap());
    spawner.spawn(update_task(stack, flash, boiler, brewing, rtc, tls).unwrap());
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
async fn update_task(stack: Stack<'static>, mut flash: FlashStorage<'static>, boiler: ActorRef<BoilerMessage>, brewing: ActorRef<BrewMessage>, rtc: &'static Rtc<'static>, tls: Tls<'static>) {
    Timer::after(Duration::from_secs(10)).await;
    confirm_running_image(&mut flash);
    stack.wait_config_up().await;

    loop {
        if synchronize_time(stack, rtc).await.is_err() {
            log::warn!("update check skipped because SNTP synchronization failed");
            Timer::after(RETRY_INTERVAL).await;
            continue;
        }

        match check_and_install(stack, &mut flash, &boiler, &brewing, &tls).await {
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

async fn synchronize_time(stack: Stack<'static>, rtc: &'static Rtc<'static>) -> Result<(), Error> {
    let addresses = stack.dns_query(NTP_SERVER, DnsQueryType::A).await.map_err(|_| Error::Time)?;
    let address = addresses.first().ok_or(Error::Time)?;
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0; 128];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
    socket.bind(0).map_err(|_| Error::Time)?;
    let socket = UdpSocketWrapper::new(socket);
    let result = get_time(SocketAddr::from((IpAddr::from(*address), 123)), &socket, NtpContext::new(Timestamp { rtc, current_time_us: 0 })).await.map_err(|_| Error::Time)?;
    rtc.set_current_time_us((result.sec() as u64 * 1_000_000) + ((result.sec_fraction() as u64 * 1_000_000) >> 32));
    Ok(())
}

async fn check_and_install(stack: Stack<'static>, flash: &mut FlashStorage<'static>, boiler: &ActorRef<BoilerMessage>, brewing: &ActorRef<BrewMessage>, tls: &Tls<'static>) -> Result<bool, Error> {
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    let certificate = Certificate::new(X509::PEM(ROOT_CA)).map_err(|_| Error::Http)?;
    let tls_config = TlsConfig::new(TlsVersion::Tls1_2, certificate, None, tls.reference());
    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls_config);
    let mut response_buffer = [0; 2048];
    let request = client.request(Method::GET, MANIFEST_URL).await.map_err(|_| Error::Http)?;
    let mut request = request.headers(&UPDATE_HEADERS);
    let response = request.send(&mut response_buffer).await.map_err(|_| Error::Http)?;
    if !response.status.is_successful() {
        return Err(Error::Http);
    }
    let body = response.body().read_to_end().await.map_err(|_| Error::Http)?;
    let manifest = manifest::parse(body).map_err(|_| Error::Manifest)?;
    drop(request);
    if !manifest.is_newer_than(env!("CARGO_PKG_VERSION")) {
        return Ok(false);
    }
    if !manifest.url.starts_with("https://gitlab.rohenkohl.dev/") {
        return Err(Error::Manifest);
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
