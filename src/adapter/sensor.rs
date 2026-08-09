use esperra_domain::{actor::ActorRef, boiler::BoilerMessage, sensor::TemperatureSensorMessage};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::{
    delay::Delay,
    gpio::{DriveMode, Flex, OutputConfig, Pin},
};
use onewire::{DS18B20, Device, OneWire};

pub struct TemperatureSensorActor<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: DS18B20,
}

impl<'d> TemperatureSensorActor<'d> {
    pub fn new(pin: impl Pin + 'd, address: [u8; 8]) -> Self {
        let mut pin = Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        Self {
            bus: OneWire::new(pin, false),
            sensor: DS18B20::new(Device { address }).unwrap(),
        }
    }

    pub async fn run<const N: usize, B>(
        &mut self,
        messages: Receiver<'_, CriticalSectionRawMutex, TemperatureSensorMessage, N>,
        boiler: &B,
    ) -> !
    where
        B: ActorRef<BoilerMessage>,
    {
        loop {
            let TemperatureSensorMessage::MeasureTemperature = messages.receive().await;
            let message = match self.read_temperature().await {
                Ok(temperature) => BoilerMessage::RegulateHeating {
                    measured_temperature: temperature,
                },
                Err(()) => BoilerMessage::StopHeating,
            };
            boiler.send(message);
        }
    }

    async fn read_temperature(&mut self) -> Result<f64, ()> {
        let mut delay = Delay::new();
        let resolution = self
            .sensor
            .measure_temperature(&mut self.bus, &mut delay)
            .map_err(|_| ())?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let raw_temperature = self
            .sensor
            .read_temperature(&mut self.bus, &mut delay)
            .map_err(|_| ())?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
