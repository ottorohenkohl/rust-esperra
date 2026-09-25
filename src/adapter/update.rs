use crate::domain::config::port::{FirmwareInfo, ReadConfigPort, WriteConfigPort};
use crate::domain::update::port::{UpdateConfirmation, UpdateError, UpdatePort};
use crate::library::network::implementation::NetworkStackImpl;

use crate::library::network::implementation::ROOT_CA;

use alloc::sync::Arc;
use core::ffi::CStr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use core::fmt::Write;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embedded_io_async::Read;
use embedded_storage::Storage;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_storage::FlashStorage;
use heapless::String;
use reqwless::client::{HttpClient, TlsConfig};
use reqwless::request::{Method, RequestBuilder};
use reqwless::{Certificate, TlsVersion};
use static_cell::StaticCell;

use core::str::from_utf8;
use esp_hal::system::software_reset;

use esp_bootloader_esp_idf::partitions::PARTITION_TABLE_MAX_LEN;
use reqwless::X509;

const UPDATE_HEADERS: [(&str, &str); 1] = [("Connection", "close")];
const IMAGE_HEADER_SIZE: usize = 80;
const FIRMWARE_BUFFER_SIZE: usize = 4096;

static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();

type FirmwareHttpClient<'a> = HttpClient<'a, TcpClient<'static, 1, 4096, 4096>, DnsSocket<'static>>;
type SharedFlash = Arc<Mutex<CriticalSectionRawMutex, FlashStorage<'static>>>;

pub struct EspUpdateAdapter<C> {
    network: &'static NetworkStackImpl,
    flash: SharedFlash,
    config: Arc<C>,
}

impl<C> EspUpdateAdapter<C> where C: ReadConfigPort + WriteConfigPort {
    pub fn initialize(flash: &mut FlashStorage<'_>) -> Result<(), UpdateError> {
        OtaUpdater::new(flash, &mut [0; PARTITION_TABLE_MAX_LEN]).map_err(|_| UpdateError::InvalidPartitionTable)?;

        Ok(())
    }

    pub fn new(network: &'static NetworkStackImpl, flash: SharedFlash, config: Arc<C>) -> Self {
        Self { network, flash, config }
    }

    async fn persist_current_version(&self) -> Result<(), UpdateError> {
        let version = String::try_from(env!("CARGO_PKG_VERSION")).map_err(|_| UpdateError::InvalidImage)?;

        let mut firmware_info = self.config.read_firmware_info();

        firmware_info.version = version;

        self.config.write_firmware_info(firmware_info).await;

        Ok(())
    }

    async fn check_and_install(&mut self) -> Result<bool, UpdateError> {
        let tcp = TcpClient::new(self.network.stack, TCP_STATE.init(TcpClientState::new()));
        let dns = DnsSocket::new(self.network.stack);
        let root_ca = CStr::from_bytes_with_nul(ROOT_CA).unwrap();
        let certificate = Certificate::new(X509::PEM(root_ca)).map_err(|_| UpdateError::Http)?;
        let tls_config = TlsConfig::new(TlsVersion::Tls1_2, certificate, None, self.network.tls.reference());

        let mut client = HttpClient::new_with_tls(&tcp, &dns, tls_config);

        let firmware_info = self.config.read_firmware_info();

        let firmware_url = Self::latest_firmware_url(&firmware_info)?;
        let latest_version = Self::fetch_latest_version(&mut client, firmware_url.as_str()).await?;

        if Self::is_newer_version(latest_version.as_str(), firmware_info.version.as_str()) {
            let mut flash = self.flash.lock().await;

            Self::install_firmware(&mut client, &mut flash, firmware_url.as_str()).await?;

            return Ok(true)
        }

        Ok(false)
    }

    async fn fetch_latest_version(client: &mut FirmwareHttpClient<'_>, firmware_url: &str) -> Result<String<32>, UpdateError> {
        let mut response_buffer = [0; 256];
        let request = client.request(Method::GET, firmware_url).await.map_err(|_| UpdateError::Http)?;
        let mut request = request.headers(&UPDATE_HEADERS);
        let response = request.send(&mut response_buffer).await.map_err(|_| UpdateError::Http)?;

        if response.status.is_successful() {
            let mut reader = response.body().reader();
            let mut image_header = [0; IMAGE_HEADER_SIZE];

            reader.read_exact(&mut image_header).await.map_err(|_| UpdateError::Http)?;

            drop(reader);
            drop(request);

            let version = Self::image_version(&image_header).ok_or(UpdateError::InvalidImage)?;

            return String::try_from(version).map_err(|_| UpdateError::InvalidImage);
        }

        Err(UpdateError::Http)
    }

    async fn install_firmware(client: &mut FirmwareHttpClient<'_>, flash: &mut FlashStorage<'static>, firmware_url: &str) -> Result<(), UpdateError> {
        let mut table = [0; PARTITION_TABLE_MAX_LEN];
        let mut updater = OtaUpdater::new(flash, &mut table).map_err(|_| UpdateError::Flash)?;

        let (mut partition, _) = updater.next_partition().map_err(|_| UpdateError::Flash)?;

        let mut response_buffer = [0; FIRMWARE_BUFFER_SIZE];
        let request = client.request(Method::GET, firmware_url).await.map_err(|_| UpdateError::Http)?;
        let mut request = request.headers(&UPDATE_HEADERS);
        let response = request.send(&mut response_buffer).await.map_err(|_| UpdateError::Http)?;

        if response.status.is_successful() {
            let mut reader = response.body().reader();
            let mut chunk = [0; FIRMWARE_BUFFER_SIZE];
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

            return Ok(());
        }

        Err(UpdateError::Http)
    }

    fn latest_firmware_url(firmware_info: &FirmwareInfo) -> Result<String<256>, UpdateError> {
        let mut url = String::new();

        write!(&mut url, "{}/latest/firmware-ota.bin", firmware_info.url.trim_end_matches('/')).map_err(|_| UpdateError::Http)?;

        Ok(url)
    }

    fn image_version(image_header: &[u8; IMAGE_HEADER_SIZE]) -> Option<&str> {
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
}

impl<C> UpdatePort for EspUpdateAdapter<C> where C: ReadConfigPort + WriteConfigPort {
    async fn update(&mut self) -> Result<UpdateConfirmation, UpdateError> {
        self.network.stack.wait_config_up().await;
        self.network.synchronize_time(self.config.read_network_info().ntp_server.as_str()).await.map_err(|_| UpdateError::Time)?;

        if self.check_and_install().await? {
            software_reset();
        }

        Ok(UpdateConfirmation::Confirmed)
    }

    async fn confirm(&mut self) -> Result<UpdateConfirmation, UpdateError> {
        let mut flash = self.flash.lock().await;

        let mut table = [0; PARTITION_TABLE_MAX_LEN];

        let mut updater = OtaUpdater::new(&mut *flash, &mut table).map_err(|_| UpdateError::Flash)?;

        if matches!(updater.current_ota_state(), Ok(OtaImageState::New | OtaImageState::PendingVerify)) {
            updater.set_current_ota_state(OtaImageState::Valid).map_err(|_| UpdateError::Flash)?;

            drop(updater);
            drop(flash);

            self.persist_current_version().await?;

            return Ok(UpdateConfirmation::Confirmed);
        }

        Ok(UpdateConfirmation::NotPending)
    }
}
