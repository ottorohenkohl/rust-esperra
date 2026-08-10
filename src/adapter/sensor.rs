use crate::domain::boiler::BoilerMessage;
use crate::domain::gpio::sensor::TemperatureSensorMessage;
use crate::library::actor::Actor;
use crate::library::actor::ActorRef;
use embassy_time::Duration;
use embassy_time::Timer;
use esp_hal::delay::Delay;
use esp_hal::gpio::DriveMode;
use esp_hal::gpio::Flex;
use esp_hal::gpio::OutputConfig;
use esp_hal::gpio::Pin;
use onewire::DS18B20;
use onewire::DeviceSearch;
use onewire::OneWire;

pub struct TemperatureSensorActor<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: DS18B20,
    boiler: ActorRef<BoilerMessage>,
}

impl<'d> TemperatureSensorActor<'d> {
    pub fn new(pin: impl Pin + 'd, boiler: ActorRef<BoilerMessage>) -> Result<Self, ()> {
        let mut pin = Flex::new(pin);
        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();

        let mut bus = OneWire::new(pin, false);
        let mut delay = Delay::new();
        let device = bus.search_next(&mut DeviceSearch::new_for_family(0x28), &mut delay).map_err(|_| ())?.ok_or(())?;
        let sensor = DS18B20::new(device).map_err(|_| ())?;

        Ok(Self { bus, sensor, boiler })
    }

    async fn read_temperature(&mut self) -> Result<f64, ()> {
        let mut delay = Delay::new();
        let resolution = self.sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| ())?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let raw_temperature = self.sensor.read_temperature(&mut self.bus, &mut delay).map_err(|_| ())?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}

impl Actor for TemperatureSensorActor<'_> {
    type Message = TemperatureSensorMessage;

    async fn handle(&mut self, message: Self::Message) {
        let TemperatureSensorMessage::MeasureTemperature = message;
        let response = match self.read_temperature().await {
            Ok(temperature) => BoilerMessage::RegulateHeating { measured_temperature: temperature },
            Err(()) => BoilerMessage::StopHeating,
        };
        self.boiler.tell(response);
    }
}
