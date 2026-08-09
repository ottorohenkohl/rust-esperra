use crate::domain::{
    actor::ActorRef,
    boiler::{BoilerMessage, TemperatureReadError},
    sensor::TemperatureSensorMessage,
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::{
    delay::Delay,
    gpio::{DriveMode, Flex, OutputConfig, Pin},
};
use onewire::{DS18B20, Device, OneWire};

pub struct TemperatureSensorActor<'d, B> {
    bus: OneWire<Flex<'d>>,
    sensor: DS18B20,
    boiler: B,
}

impl<'d, B> TemperatureSensorActor<'d, B>
where
    B: ActorRef<BoilerMessage>,
{
    pub fn new(pin: impl Pin + 'd, address: [u8; 8], boiler: B) -> Self {
        let mut pin = Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        Self {
            bus: OneWire::new(pin, false),
            sensor: DS18B20::new(Device { address }).unwrap(),
            boiler,
        }
    }

    pub async fn run(
        &mut self,
        messages: Receiver<'_, CriticalSectionRawMutex, TemperatureSensorMessage, 1>,
    ) -> ! {
        loop {
            let TemperatureSensorMessage::Measure = messages.receive().await;
            let _ = self.boiler.try_send(BoilerMessage::TemperatureMeasured(
                self.read_temperature().await,
            ));
        }
    }

    async fn read_temperature(&mut self) -> Result<f64, TemperatureReadError> {
        let mut delay = Delay::new();
        let resolution = self
            .sensor
            .measure_temperature(&mut self.bus, &mut delay)
            .map_err(|_| TemperatureReadError::ReadFailed)?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let raw_temperature = self
            .sensor
            .read_temperature(&mut self.bus, &mut delay)
            .map_err(|_| TemperatureReadError::ReadFailed)?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
