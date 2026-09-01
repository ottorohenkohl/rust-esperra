use crate::domain::config::port::{ConfigError, ConfigPort, FirmwareInfo, HeaterOptions, NetworkInfo, PidGains};

use embedded_storage::{ReadStorage, Storage};
use esp_storage::FlashStorage;
use heapless::String;

use core::str::from_utf8;

const CONFIG_STORAGE_OFFSET: u32 = 0x9000;
const MAGIC_RECORD: u16 = 2307;
const RECORD_SIZE: usize = 512;

pub struct FlashConfigPort {
    firmware_info: FirmwareInfo,
    heater_options: HeaterOptions,
    network_info: NetworkInfo,
    pid_gains: PidGains,
}

impl FlashConfigPort {
    pub fn new(storage: &mut FlashStorage<'_>) -> Result<Self, ConfigError> {
        let mut record = [0; RECORD_SIZE];

        storage.read(CONFIG_STORAGE_OFFSET, &mut record).map_err(|_| ConfigError::StorageReadFailed)?;

        if Self::read_u16(&record, 0) == MAGIC_RECORD {
            let firmware_info = FirmwareInfo {
                url: Self::read_string(&record, 60, 256),
                version: Self::read_string(&record, 316, 32),
            };
            let heater_options = HeaterOptions {
                heating_cycle_ms: Self::read_u64(&record, 4),
                target_temperature: Self::read_f64(&record, 12),
                target_temperature_interval: Self::read_f64(&record, 20),
                boost_intervall_size: Self::read_f64(&record, 28),
            };
            let network_info = NetworkInfo {
                ntp_server: Self::read_string(&record, 348, 64),
                wifi_ssid: Self::read_string(&record, 412, 32),
                wifi_password: Self::read_string(&record, 444, 64),
            };
            let pid_gains = PidGains {
                k_proportional: Self::read_f64(&record, 36),
                k_intergral: Self::read_f64(&record, 44),
                k_derivative: Self::read_f64(&record, 52),
            };

            return Ok(Self { firmware_info, heater_options, network_info, pid_gains });
        }

        let config = Self {
            firmware_info: Self::default_firmware_info(),
            heater_options: Self::default_heater_options(),
            network_info: Self::default_network_info(),
            pid_gains: Self::default_pid_gains(),
        };

        storage.write(CONFIG_STORAGE_OFFSET, &config.record()).map_err(|_| ConfigError::StorageWriteFailed)?;

        Ok(config)
    }

    fn read_u16(record: &[u8; RECORD_SIZE], offset: usize) -> u16 {
        u16::from_le_bytes(record[offset..offset + 2].try_into().unwrap())
    }

    fn read_u64(record: &[u8; RECORD_SIZE], offset: usize) -> u64 {
        u64::from_le_bytes(record[offset..offset + 8].try_into().unwrap())
    }

    fn read_f64(record: &[u8; RECORD_SIZE], offset: usize) -> f64 {
        f64::from_le_bytes(record[offset..offset + 8].try_into().unwrap())
    }

    fn read_string<const SIZE: usize>(record: &[u8; RECORD_SIZE], offset: usize, length: usize) -> String<SIZE> {
        let bytes = &record[offset..offset + length];
        let length = bytes.iter().position(|byte| *byte == 0 || *byte == 0xff).unwrap_or(bytes.len());

        from_utf8(&bytes[..length]).ok().and_then(|value| String::try_from(value).ok()).unwrap_or_default()
    }

    fn record(&self) -> [u8; RECORD_SIZE] {
        let mut record = [0; RECORD_SIZE];

        Self::write_u16(&mut record, 0, MAGIC_RECORD);
        Self::write_u64(&mut record, 4, self.heater_options.heating_cycle_ms);
        Self::write_f64(&mut record, 12, self.heater_options.target_temperature);
        Self::write_f64(&mut record, 20, self.heater_options.target_temperature_interval);
        Self::write_f64(&mut record, 28, self.heater_options.boost_intervall_size);
        Self::write_f64(&mut record, 36, self.pid_gains.k_proportional);
        Self::write_f64(&mut record, 44, self.pid_gains.k_intergral);
        Self::write_f64(&mut record, 52, self.pid_gains.k_derivative);
        Self::write_string(&mut record, 60, &self.firmware_info.url);
        Self::write_string(&mut record, 316, &self.firmware_info.version);
        Self::write_string(&mut record, 348, &self.network_info.ntp_server);
        Self::write_string(&mut record, 412, &self.network_info.wifi_ssid);
        Self::write_string(&mut record, 444, &self.network_info.wifi_password);

        record
    }

    fn write_u16(record: &mut [u8; RECORD_SIZE], offset: usize, value: u16) {
        record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(record: &mut [u8; RECORD_SIZE], offset: usize, value: u64) {
        record[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_f64(record: &mut [u8; RECORD_SIZE], offset: usize, value: f64) {
        record[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_string<const SIZE: usize>(record: &mut [u8; RECORD_SIZE], offset: usize, value: &String<SIZE>) {
        record[offset..offset + value.len()].copy_from_slice(value.as_bytes());
    }

    fn default_heater_options() -> HeaterOptions {
        let heating_cycle_ms = option_env!("ESPERRA_HEATER_OPTIONS_HEATING_CYCLE_MS").unwrap_or("100").parse().unwrap();
        let target_temperature = option_env!("ESPERRA_HEATER_OPTIONS_TARGET_TEMPERATURE").unwrap_or("94.0").parse().unwrap();
        let target_temperature_interval = option_env!("ESPERRA_HEATER_OPTIONS_TARGET_TEMPERATURE_INTERVAL").unwrap_or("0.5").parse().unwrap();
        let boost_intervall_size = option_env!("ESPERRA_HEATER_OPTIONS_BOOST_INTERVAL_SIZE").unwrap_or("5.0").parse().unwrap();

        HeaterOptions { heating_cycle_ms, target_temperature, target_temperature_interval, boost_intervall_size }
    }

    fn default_pid_gains() -> PidGains {
        let k_proportional = option_env!("ESPERRA_PID_GAINS_K_PROPORTIONAL").unwrap_or("0.1").parse().unwrap();
        let k_intergral = option_env!("ESPERRA_PID_GAINS_K_INTERGRAL").unwrap_or("0.2").parse().unwrap();
        let k_derivative = option_env!("ESPERRA_PID_GAINS_K_DERIVATIVE").unwrap_or("0.3").parse().unwrap();

        PidGains { k_proportional, k_intergral, k_derivative }
    }

    fn default_firmware_info() -> FirmwareInfo {
        let url = String::try_from(option_env!("ESPERRA_FIRMWARE_URL").unwrap_or("")).unwrap();
        let version = String::try_from(env!("CARGO_PKG_VERSION")).unwrap();

        FirmwareInfo { url, version }
    }

    fn default_network_info() -> NetworkInfo {
        let ntp_server = String::try_from(option_env!("ESPERRA_NTP_SERVER").unwrap_or("pool.ntp.org")).unwrap();
        let wifi_ssid = String::try_from(option_env!("ESPERRA_WIFI_SSID").unwrap_or("Simulation")).unwrap();
        let wifi_password = String::try_from(option_env!("ESPERRA_WIFI_PASSWORD").unwrap_or("password")).unwrap();

        NetworkInfo { ntp_server, wifi_ssid, wifi_password }
    }
}

impl ConfigPort for FlashConfigPort {
    fn read_heater_options(&self) -> &HeaterOptions {
        &self.heater_options
    }

    fn read_pid_gains(&self) -> &PidGains {
        &self.pid_gains
    }

    fn read_firmware_info(&self) -> &FirmwareInfo {
        &self.firmware_info
    }

    fn read_network_info(&self) -> &NetworkInfo {
        &self.network_info
    }
}
