use crate::domain::config::port::{ConfigPort, FirmwareInfo, HeaterOptions, PidGains};

use embedded_storage::ReadStorage;
use esp_storage::{FlashStorage, FlashStorageError};
use heapless::String;

const CONFIG_STORAGE_OFFSET: u32 = 0x9000;
const MAGIC_RECORD: u16 = 2307;
const RECORD_SIZE: usize = 348;

pub struct FlashConfigPort {
    heater_options: HeaterOptions,
    pid_gains: PidGains,
    firmware_info: FirmwareInfo,
}

impl FlashConfigPort {
    pub fn new(storage: &mut FlashStorage<'_>) -> Result<Self, FlashStorageError> {
        let mut record = [0; RECORD_SIZE];

        storage.read(CONFIG_STORAGE_OFFSET, &mut record)?;

        let mut heater_options = Self::default_heater_options();
        let mut pid_gains = Self::default_pid_gains();
        let mut firmware_info = Self::default_firmware_info();

        if Self::read_u16(&record, 0) == MAGIC_RECORD {
            heater_options = HeaterOptions {
                heating_cycle_ms: Self::read_u64(&record, 4),
                target_temperature: Self::read_f64(&record, 12),
                target_temperature_interval: Self::read_f64(&record, 20),
                boost_intervall_size: Self::read_f64(&record, 28),
            };

            pid_gains = PidGains {
                k_proportional: Self::read_f64(&record, 36),
                k_intergral: Self::read_f64(&record, 44),
                k_derivative: Self::read_f64(&record, 52),
            };

            firmware_info = FirmwareInfo {
                url: Self::read_string(&record, 60, 256),
                version: Self::read_string(&record, 316, 32),
            };
        }

        Ok(Self { heater_options, pid_gains, firmware_info })
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

    fn read_string<const N: usize>(record: &[u8; RECORD_SIZE], offset: usize, length: usize) -> String<N> {
        let bytes = &record[offset..offset + length];
        let length = bytes.iter().position(|byte| *byte == 0 || *byte == 0xff).unwrap_or(bytes.len());

        core::str::from_utf8(&bytes[..length]).ok().and_then(|value| String::try_from(value).ok()).unwrap_or_default()
    }

    fn default_heater_options() -> HeaterOptions {
        let heating_cycle_ms = String::try_from(option_env!("ESPERRA_HEATER_OPTIONS_HEATING_CYCLE_MS").unwrap_or("")).unwrap();
        let target_temperature = String::try_from(option_env!("ESPERRA_HEATER_OPTIONS_TARGET_TEMPERATURE").unwrap_or("")).unwrap();
        let target_temperature_interval = String::try_from(option_env!("ESPERRA_HEATER_OPTIONS_TARGET_TEMPERATURE_INTERVAL").unwrap_or("")).unwrap();
        let boost_intervall_size = String::try_from(option_env!("ESPERRA_HEATER_OPTIONS_BOOST_INTERVAL_SIZE").unwrap_or("")).unwrap();

        HeaterOptions { heating_cycle_ms, target_temperature, target_temperature_interval, boost_intervall_size }
    }

    fn default_pid_gains() -> PidGains {
        let k_proportional = String::try_from(option_env!("ESPERRA_PID_GAINS_K_PROPORTIONAL").unwrap_or("")).unwrap();
        let k_intergral = String::try_from(option_env!("ESPERRA_PID_GAINS_K_INTERGRAL").unwrap_or("")).unwrap();
        let k_derivative = String::try_from(option_env!("ESPERRA_PID_GAINS_K_DERIVATIVE").unwrap_or("")).unwrap();

        PidGains { k_proportional, k_intergral, k_derivative }
    }

    fn default_firmware_info() -> FirmwareInfo {
        let url = String::try_from(option_env!("ESPERRA_FIRMWARE_INFO_URL").unwrap_or("")).unwrap();
        let version = String::try_from(env!("ESPERRA_FIRMWARE_INFO_VERSION")).unwrap();

        FirmwareInfo { url, version }
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
}
