use core::ffi::CStr;
use core::fmt::Write;
use core::net::{IpAddr, SocketAddr};

use alloc::sync::Arc;

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
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use static_cell::StaticCell;
use heapless::String;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::{ConfigPort, FirmwareInfo};
use crate::domain::update::port::UpdatePort;
use crate::library::actor::reference::ActorRef;

const UPDATE_HEADERS: [(&str, &str); 1] = [("Connection", "close")];
const ROOT_CA: &CStr = match CStr::from_bytes_with_nul(concat!(include_str!("../../res/isrg-root-x1.pem"), "\0").as_bytes()) {
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
pub enum UpdateError {
    Flash,
    Http,
    InvalidImage,
    Time,
}

pub struct EspUpdatePort<C> {
    stack: Stack<'static>,
    flash: FlashStorage<'static>,
    config: Arc<C>,
    boiler: ActorRef<BoilerMessage>,
    brewing: ActorRef<BrewMessage>,
    rtc: &'static Rtc<'static>,
    tls: Tls<'static>,
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

impl<C> EspUpdatePort<C>
where
    C: ConfigPort,
{
    pub fn initialize(flash: &mut FlashStorage<'_>) {
        let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
        if OtaUpdater::new(flash, &mut table).is_err() {
            log::error!("invalid OTA partition table");
        }
    }

    pub fn new(
        wifi: esp_hal::peripherals::WIFI<'static>,
        lpwr: esp_hal::peripherals::LPWR<'static>,
        rng: esp_hal::peripherals::RNG<'static>,
        adc1: esp_hal::peripherals::ADC1<'static>,
        flash: FlashStorage<'static>,
        config: Arc<C>,
        boiler: ActorRef<BoilerMessage>,
        brewing: ActorRef<BrewMessage>,
        spawner: embassy_executor::Spawner,
    ) -> Option<Self> {
        let firmware_info = config.read_firmware_info();
        let network_info = config.read_network_info();
        if firmware_info.url.is_empty() || network_info.wifi_ssid.is_empty() {
            return None;
        }

        let station = Config::Station(StationConfig::default().with_ssid(network_info.wifi_ssid.as_str()).with_password(network_info.wifi_password.as_str().into()));
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

        Some(Self { stack, flash, config, boiler, brewing, rtc, tls })
    }
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

impl<C> UpdatePort for EspUpdatePort<C>
where
    C: ConfigPort,
{
    type Error = UpdateError;

    async fn update(&mut self) -> Result<(), Self::Error> {
        self.stack.wait_config_up().await;
        synchronize_time(self.stack, self.rtc, self.config.read_network_info().ntp_server.as_str()).await?;
        if check_and_install(self.stack, &mut self.flash, self.config.read_firmware_info(), &self.boiler, &self.brewing, &self.tls).await? {
            esp_hal::system::software_reset();
        }

        Ok(())
    }

    fn confirm(&mut self) -> Result<bool, Self::Error> {
        let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
        let mut updater = OtaUpdater::new(&mut self.flash, &mut table).map_err(|_| UpdateError::Flash)?;
        if matches!(updater.current_ota_state(), Ok(OtaImageState::New | OtaImageState::PendingVerify)) {
            updater.set_current_ota_state(OtaImageState::Valid).map_err(|_| UpdateError::Flash)?;
            log::info!("confirmed OTA image {}", env!("CARGO_PKG_VERSION"));
            return Ok(true);
        }

        Ok(false)
    }
}

async fn synchronize_time(stack: Stack<'static>, rtc: &'static Rtc<'static>, ntp_server: &str) -> Result<(), UpdateError> {
    let addresses = stack.dns_query(ntp_server, DnsQueryType::A).await.map_err(|_| UpdateError::Time)?;
    let address = addresses.first().ok_or(UpdateError::Time)?;
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0; 128];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0; 128];
    let mut socket = UdpSocket::new(stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);
    socket.bind(0).map_err(|_| UpdateError::Time)?;
    let socket = UdpSocketWrapper::new(socket);
    let result = get_time(SocketAddr::from((IpAddr::from(*address), 123)), &socket, NtpContext::new(Timestamp { rtc, current_time_us: 0 })).await.map_err(|_| UpdateError::Time)?;
    rtc.set_current_time_us((result.sec() as u64 * 1_000_000) + ((result.sec_fraction() as u64 * 1_000_000) >> 32));
    Ok(())
}

async fn check_and_install(stack: Stack<'static>, flash: &mut FlashStorage<'static>, firmware_info: &FirmwareInfo, boiler: &ActorRef<BoilerMessage>, brewing: &ActorRef<BrewMessage>, tls: &Tls<'static>) -> Result<bool, UpdateError> {
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    let certificate = Certificate::new(X509::PEM(ROOT_CA)).map_err(|_| UpdateError::Http)?;
    let tls_config = TlsConfig::new(TlsVersion::Tls1_2, certificate, None, tls.reference());
    let mut client = HttpClient::new_with_tls(&tcp, &dns, tls_config);

    let latest_firmware_url = latest_firmware_url(firmware_info)?;
    let mut header_buffer = [0; 256];
    let request = client.request(Method::GET, latest_firmware_url.as_str()).await.map_err(|_| UpdateError::Http)?;
    let mut request = request.headers(&UPDATE_HEADERS);
    let response = request.send(&mut header_buffer).await.map_err(|_| UpdateError::Http)?;
    if !response.status.is_successful() {
        return Err(UpdateError::Http);
    }
    let mut reader = response.body().reader();
    let mut image_header = [0; 80];
    reader.read_exact(&mut image_header).await.map_err(|_| UpdateError::Http)?;
    drop(reader);
    drop(request);

    let version = image_version(&image_header).ok_or(UpdateError::InvalidImage)?;
    if !is_newer_version(version, firmware_info.version.as_str()) {
        return Ok(false);
    }

    boiler.tell(BoilerMessage::SafetyShutdown);
    brewing.tell(BrewMessage::SetBrewing(false));
    Timer::after(Duration::from_secs(2)).await;

    let mut table = [0; esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN];
    let mut updater = OtaUpdater::new(flash, &mut table).map_err(|_| UpdateError::Flash)?;
    let (mut partition, _) = updater.next_partition().map_err(|_| UpdateError::Flash)?;

    let mut firmware_buffer = [0; 4096];
    let request = client.request(Method::GET, latest_firmware_url.as_str()).await.map_err(|_| UpdateError::Http)?;
    let mut request = request.headers(&UPDATE_HEADERS);
    let response = request.send(&mut firmware_buffer).await.map_err(|_| UpdateError::Http)?;
    if !response.status.is_successful() {
        return Err(UpdateError::Http);
    }

    let mut reader = response.body().reader();
    let mut chunk = [0; 4096];
    let mut offset = 0;
    loop {
        let read = reader.read(&mut chunk).await.map_err(|_| UpdateError::Http)?;
        if read == 0 {
            break;
        }
        if offset + read > partition.partition_size() {
            return Err(UpdateError::InvalidImage);
        }
        partition.write(offset as u32, &chunk[..read]).map_err(|_| UpdateError::Flash)?;
        offset += read;
    }

    if offset == 0 {
        return Err(UpdateError::InvalidImage);
    }
    drop(partition);
    updater.activate_next_partition().map_err(|_| UpdateError::Flash)?;
    updater.set_current_ota_state(OtaImageState::New).map_err(|_| UpdateError::Flash)?;
    Ok(true)
}

fn latest_firmware_url(firmware_info: &FirmwareInfo) -> Result<String<256>, UpdateError> {
    let mut url = String::new();
    write!(&mut url, "{}/latest/firmware-ota.bin", firmware_info.url.trim_end_matches('/')).map_err(|_| UpdateError::Http)?;
    Ok(url)
}

fn image_version(image_header: &[u8; 80]) -> Option<&str> {
    const APP_DESC_OFFSET: usize = 32;
    const VERSION_OFFSET: usize = APP_DESC_OFFSET + 16;
    const VERSION_SIZE: usize = 32;

    if u32::from_le_bytes(image_header[APP_DESC_OFFSET..APP_DESC_OFFSET + 4].try_into().ok()?) != 0xabcd_5432 {
        return None;
    }
    let version = &image_header[VERSION_OFFSET..VERSION_OFFSET + VERSION_SIZE];
    let length = version.iter().position(|byte| *byte == 0).unwrap_or(VERSION_SIZE);
    core::str::from_utf8(&version[..length]).ok()
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    let mut candidate = candidate.split('.');
    let mut current = current.split('.');

    for _ in 0..3 {
        let (Some(candidate), Some(current)) = (candidate.next(), current.next()) else { return false };
        match candidate.parse::<u32>().ok().zip(current.parse::<u32>().ok()) {
            Some((candidate, current)) if candidate > current => return true,
            Some((candidate, current)) if candidate < current => return false,
            Some(_) => {}
            None => return false,
        }
    }

    candidate.next().is_none() && current.next().is_none()
}
