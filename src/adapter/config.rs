use crate::domain::config::port::{ConfigPort, HeaterOptions, PidGains};
use embedded_storage::ReadStorage;
use esp_storage::{FlashStorage, FlashStorageError};

const CONFIG_STORAGE_OFFSET: u32 = 0x9000;
const MAGIC_RECORD: u16 = 2307;
const RECORD_SIZE: usize = 60;

pub struct FlashConfigPort {
    heater_options: HeaterOptions,
    pid_gains: PidGains,
}

impl FlashConfigPort {
    pub fn new(storage: &mut FlashStorage<'_>) -> Result<Self, FlashStorageError> {

        let mut record = [0; RECORD_SIZE];

        storage.read(CONFIG_STORAGE_OFFSET, &mut record)?;

        let mut heater_options = Self::default_heater_options();
        let mut pid_gains = Self::default_pid_gains();

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
        }

        Ok(Self { heater_options, pid_gains })
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

    fn default_heater_options() -> HeaterOptions {
        HeaterOptions { heating_cycle_ms: 100_000, target_temperature: 65.0, target_temperature_interval: 0.5, boost_intervall_size: 5.0 }
    }

    fn default_pid_gains() -> PidGains {
        PidGains { k_proportional: 1.0, k_intergral: 0.0, k_derivative: 0.0 }
    }
}

impl ConfigPort for FlashConfigPort {
    fn read_heater_options(&self) -> &HeaterOptions {
        &self.heater_options
    }

    fn read_pid_gains(&self) -> &PidGains {
        &self.pid_gains
    }
}
