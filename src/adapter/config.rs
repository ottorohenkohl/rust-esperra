use crate::domain::config::port::{ConfigPort, HeaterOptions, PidGains};

use embedded_storage::{ReadStorage, Storage};
use esp_storage::{FlashStorage, FlashStorageError};

use esp_hal::peripherals::FLASH;

const MAGIC_RECORD: u16 = 2307;
const RECORD_SIZE: usize = 52;

pub struct FlashConfigPort<'d> {
    storage: FlashStorage<'d>,

    heater_options: HeaterOptions,
    pid_gains: PidGains,
    storage_offset: u32,
}

impl<'d> FlashConfigPort<'d> {
    pub fn new(flash: FLASH<'d>, storage_offset: u32) -> Result<Self, FlashStorageError> {
        let mut storage = FlashStorage::new(flash);

        let mut record = [0; RECORD_SIZE];

        storage.read(storage_offset, &mut record)?;

        let mut heater_options = Self::default_heater_options();
        let mut pid_gains = Self::default_pid_gains();

        if Self::read_u16(&record, 0) == MAGIC_RECORD {
            heater_options = HeaterOptions {
                heating_cycle_ms: Self::read_u64(&record, 4),
                target_temperature: Self::read_f64(&record, 12),
                boost_intervall_size: Self::read_f64(&record, 20),
            };

            pid_gains = PidGains {
                k_proportional: Self::read_f64(&record, 28),
                k_intergral: Self::read_f64(&record, 36),
                k_derivative: Self::read_f64(&record, 44),
            };
        }

        Ok(Self {
            storage,
            storage_offset,
            heater_options,
            pid_gains,
        })
    }

    pub fn save(
        &mut self,
        heater_options: HeaterOptions,
        pid_gains: PidGains,
    ) -> Result<(), FlashStorageError> {
        let record = Self::encode(heater_options, pid_gains);

        self.storage.write(self.storage_offset, &record)?;

        self.heater_options = heater_options;
        self.pid_gains = pid_gains;

        Ok(())
    }

    fn encode(heater_options: HeaterOptions, pid_gains: PidGains) -> [u8; RECORD_SIZE] {
        let mut record = [0xff; RECORD_SIZE];

        record[..2].copy_from_slice(&MAGIC_RECORD.to_le_bytes());

        record[4..12].copy_from_slice(&heater_options.heating_cycle_ms.to_le_bytes());
        record[12..20].copy_from_slice(&heater_options.target_temperature.to_le_bytes());
        record[20..28].copy_from_slice(&heater_options.boost_intervall_size.to_le_bytes());
        record[28..36].copy_from_slice(&pid_gains.k_proportional.to_le_bytes());
        record[36..44].copy_from_slice(&pid_gains.k_intergral.to_le_bytes());
        record[44..52].copy_from_slice(&pid_gains.k_derivative.to_le_bytes());

        record
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
        HeaterOptions {
            heating_cycle_ms: 100_000,
            target_temperature: 65.0,
            boost_intervall_size: 5.0,
        }
    }

    fn default_pid_gains() -> PidGains {
        PidGains {
            k_proportional: 1.0,
            k_intergral: 0.0,
            k_derivative: 0.0,
        }
    }
}

impl ConfigPort for FlashConfigPort<'_> {
    fn read_heater_options(&self) -> &HeaterOptions {
        &self.heater_options
    }

    fn read_pid_gains(&self) -> &PidGains {
        &self.pid_gains
    }
}
