use crate::domain::config::port::{ConfigPort, FirmwareInfo};
use crate::domain::update::port::{UpdateConfirmation, UpdateError, UpdatePort};

use alloc::sync::Arc;
use core::ffi::CStr;
use core::fmt::Write;
use core::net::{IpAddr, SocketAddr};
use embassy_executor::{Spawner};
use embassy_net::dns::{DnsQueryType, DnsSocket};
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use embedded_storage::Storage;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_hal::rng::{Rng, Trng, TrngSource};
use esp_hal::rtc_cntl::Rtc;
use esp_radio::wifi::{ControllerConfig, Interface, WifiController};
use esp_radio::wifi::sta::StationConfig;
use esp_storage::FlashStorage;
use heapless::String;
use mbedtls_rs::Tls;
use mbedtls_rs::sys::hook::backend::embassy::timer::EmbassyTimer;
use mbedtls_rs::sys::hook::backend::esp::wall_clock::EspRtcWallClock;
use reqwless::client::{HttpClient, TlsConfig};
use reqwless::request::{Method, RequestBuilder};
use reqwless::{Certificate, TlsVersion};
use sntpc::NtpContext;
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;
use static_cell::StaticCell;

use embassy_net::Config as NetworkConfig;
use esp_radio::wifi::Config as WifiConfig;

use core::str::from_utf8;
use embassy_executor::task;
use embassy_net;
use esp_radio::wifi;
use mbedtls_rs::sys::hook::{timer::hook_timer, wall_clock::hook_wall_clock};
use sntpc::get_time;
use esp_hal::system::software_reset;

use esp_hal::peripherals::{ADC1, LPWR, RNG, WIFI};
use esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN;
use reqwless::X509;

const UPDATE_HEADERS: [(&str, &str); 1] = [("Connection", "close")];
const ROOT_CA: &[u8] = concat!(include_str!("../../res/isrg-root-x1.pem"), "\0").as_bytes();

static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();
static RTC: StaticCell<Rtc<'static>> = StaticCell::new();
static TRNG_SOURCE: StaticCell<TrngSource<'static>> = StaticCell::new();
static TRNG: StaticCell<Trng> = StaticCell::new();
static TLS_TIMER: StaticCell<EmbassyTimer> = StaticCell::new();
static TLS_CLOCK: StaticCell<EspRtcWallClock<&'static Rtc<'static>>> = StaticCell::new();

pub struct EspUpdatePort<C> {
    stack: Stack<'static>,
    flash: FlashStorage<'static>,
    config: Arc<C>,
    rtc: &'static Rtc<'static>,
    tls: Tls<'static>,
}

impl<C> EspUpdatePort<C> where C: ConfigPort {
    pub fn initialize(flash: &mut FlashStorage<'_>) -> Result<(), UpdateError> {
        OtaUpdater::new(flash, &mut [0; PARTITION_TABLE_MAX_LEN]).map_err(|_| UpdateError::InvalidPartitionTable)?;

        Ok(())
    }

    pub fn new(wifi: WIFI<'static>, lpwr: LPWR<'static>, rng: RNG<'static>, adc1: ADC1<'static>, flash: FlashStorage<'static>, config: Arc<C>, spawner: Spawner) -> Self {
        let network_info = config.read_network_info();

        let station = WifiConfig::Station(StationConfig::default().with_ssid(network_info.wifi_ssid.as_str()).with_password(network_info.wifi_password.as_str().into()));
        let controller_config = ControllerConfig::default().with_initial_config(station);
        let network_config = NetworkConfig::dhcpv4(Default::default());
        let random = Rng::new();

        let (controller, interfaces) = wifi::new(wifi, controller_config).unwrap();
        let (stack, runner) = embassy_net::new(interfaces.station, network_config, STACK_RESOURCES.init(StackResources::new()), (random.random() as u64) << 32 | random.random() as u64);

        let rtc = RTC.init(Rtc::new(lpwr));

        TRNG_SOURCE.init(TrngSource::new(rng, adc1));

        let trng = TRNG.init(Trng::try_new().unwrap());

        unsafe {
            hook_timer(Some(TLS_TIMER.init(EmbassyTimer)));
            hook_wall_clock(Some(TLS_CLOCK.init(EspRtcWallClock::new(rtc))));
        }

        let tls = Tls::new(trng).unwrap();

        spawner.spawn(connection_task(controller).unwrap());
        spawner.spawn(network_task(runner).unwrap());

        Self { stack, flash, config, rtc, tls }
    }
}

impl<C> UpdatePort for EspUpdatePort<C> where C: ConfigPort {
    async fn update(&mut self) -> Result<(), UpdateError> {
        self.stack.wait_config_up().await;

        synchronize_time(self.stack, self.rtc, self.config.read_network_info().ntp_server.as_str()).await?;

        if check_and_install(self.stack, &mut self.flash, self.config.read_firmware_info(), &self.tls).await? {
            software_reset();
        }

        Ok(())
    }

    fn confirm(&mut self) -> Result<UpdateConfirmation, UpdateError> {
        let mut table = [0; PARTITION_TABLE_MAX_LEN];

        let mut updater = OtaUpdater::new(&mut self.flash, &mut table).map_err(|_| UpdateError::Flash)?;

        if matches!(updater.current_ota_state(), Ok(OtaImageState::New | OtaImageState::PendingVerify)) {
            updater.set_current_ota_state(OtaImageState::Valid).map_err(|_| UpdateError::Flash)?;

            return Ok(UpdateConfirmation::Confirmed);
        }

        Ok(UpdateConfirmation::NotPending)
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

    let result = get_time(SocketAddr::from((IpAddr::from(*address), 123)), &socket, NtpContext::new(EmbassyTimestampGenerator::default())).await.map_err(|_| UpdateError::Time)?;

    rtc.set_current_time_us((result.sec() as u64 * 1_000_000) + ((result.sec_fraction() as u64 * 1_000_000) >> 32));

    Ok(())
}

#[task]
async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        if controller.connect_async().await.is_ok() {
            let _ = controller.wait_for_disconnect_async().await;
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

#[task]
async fn network_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await;
}

async fn check_and_install(stack: Stack<'static>, flash: &mut FlashStorage<'static>, firmware_info: &FirmwareInfo, tls: &Tls<'static>) -> Result<bool, UpdateError> {
    let tcp = TcpClient::new(stack, TCP_STATE.init(TcpClientState::new()));
    let dns = DnsSocket::new(stack);
    let root_ca = CStr::from_bytes_with_nul(ROOT_CA).unwrap();
    let certificate = Certificate::new(X509::PEM(root_ca)).map_err(|_| UpdateError::Http)?;
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

    let mut table = [0; PARTITION_TABLE_MAX_LEN];

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

    from_utf8(&version[..length]).ok()
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
