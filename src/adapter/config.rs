use crate::domain::config::port::{ConfigProvider, HeaterOptions, PidGains};

use embedded_storage::{ReadStorage, Storage};
use esp_hal::peripherals::FLASH;
use esp_storage::{FlashStorage, FlashStorageError};

const MAGIC: [u8; 4] = *b"ESPR";
const VERSION: u16 = 1;
const RECORD_SIZE: usize = 56;

pub struct FlashConfigProvider<'d> {
    storage: FlashStorage<'d>,
    storage_offset: u32,
    heater_options: HeaterOptions,
    pid_gains: PidGains,
}

impl<'d> FlashConfigProvider<'d> {
    pub fn new(flash: FLASH<'d>, storage_offset: u32) -> Result<Self, FlashStorageError> {
        let mut storage = FlashStorage::new(flash);
        let (heater_options, pid_gains) = Self::read(&mut storage, storage_offset)?;

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

    fn read(
        storage: &mut FlashStorage<'d>,
        storage_offset: u32,
    ) -> Result<(HeaterOptions, PidGains), FlashStorageError> {
        let mut record = [0; RECORD_SIZE];
        storage.read(storage_offset, &mut record)?;

        if record[..4] != MAGIC || Self::read_u16(&record, 4) != VERSION {
            return Ok((Self::default_heater_options(), Self::default_pid_gains()));
        }

        Ok((
            HeaterOptions {
                heating_cycle_ms: Self::read_u64(&record, 8),
                target_temperature: Self::read_f64(&record, 16),
                boost_intervall_size: Self::read_f64(&record, 24),
            },
            PidGains {
                k_proportional: Self::read_f64(&record, 32),
                k_intergral: Self::read_f64(&record, 40),
                k_derivative: Self::read_f64(&record, 48),
            },
        ))
    }

    fn encode(heater_options: HeaterOptions, pid_gains: PidGains) -> [u8; RECORD_SIZE] {
        let mut record = [0xff; RECORD_SIZE];
        record[..4].copy_from_slice(&MAGIC);
        record[4..6].copy_from_slice(&VERSION.to_le_bytes());
        record[8..16].copy_from_slice(&heater_options.heating_cycle_ms.to_le_bytes());
        record[16..24].copy_from_slice(&heater_options.target_temperature.to_le_bytes());
        record[24..32].copy_from_slice(&heater_options.boost_intervall_size.to_le_bytes());
        record[32..40].copy_from_slice(&pid_gains.k_proportional.to_le_bytes());
        record[40..48].copy_from_slice(&pid_gains.k_intergral.to_le_bytes());
        record[48..56].copy_from_slice(&pid_gains.k_derivative.to_le_bytes());
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

impl ConfigProvider for FlashConfigProvider<'_> {
    fn read_heater_options(&self) -> &HeaterOptions {
        &self.heater_options
    }

    fn read_pid_gains(&self) -> &PidGains {
        &self.pid_gains
    }
}
