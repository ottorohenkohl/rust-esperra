use crate::domain::gpio::port::{InputPort, OutputPort, TemperatureSensorError, TemperatureSensorPort};

use embassy_time::{Duration, Timer};
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, InputPin, Level, Output, OutputConfig, OutputPin, Pull, DriveMode, Flex, Pin};
use onewire::{DeviceSearch, OneWire};

use onewire::DS18B20;

pub struct EspOutputAdapter<'d> {
    pin: Output<'d>,
    active_low: bool,
}

impl<'d> EspOutputAdapter<'d> {
    pub fn new(pin: impl OutputPin + 'd, active_low: bool) -> Self {
        let inactive_level = if active_low { Level::High } else { Level::Low };

        let pin = Output::new(pin, inactive_level, OutputConfig::default());

        Self { pin, active_low }
    }
}

impl OutputPort for EspOutputAdapter<'_> {
    async fn set_active(&mut self, active: bool) {
        let high = active != self.active_low;

        if high {
            self.pin.set_high();
        } else {
            self.pin.set_low();
        }
    }

    async fn invert_state(&mut self) -> bool {
        let active = self.pin.is_set_low() != self.active_low;

        self.set_active(active).await;

        active
    }
}

pub struct EspInputAdapter<'d> {
    pin: Input<'d>,
}

impl<'d> EspInputAdapter<'d> {
    pub fn new(pin: impl InputPin + 'd, pull: Pull) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(pull));

        Self { pin }
    }
}

impl InputPort for EspInputAdapter<'_> {
    fn powered_state(&self) -> bool {
        self.pin.is_low()
    }
}

pub struct Ds18b20TemperatureAdapter<'d> {
    bus: OneWire<Flex<'d>>,
    sensor: Option<DS18B20>,
}

impl<'d> Ds18b20TemperatureAdapter<'d> {
    pub fn new(pin: impl Pin + 'd) -> Self {
        let mut pin = Flex::new(pin);

        pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
        pin.set_input_enable(true);
        pin.set_high();
        pin.set_output_enable(true);

        let mut bus = OneWire::new(pin, false);
        let sensor = Self::discover(&mut bus).ok();

        Self { bus, sensor }
    }


    fn discover(bus: &mut OneWire<Flex<'d>>) -> Result<DS18B20, TemperatureSensorError> {
        let mut delay = Delay::new();

        let presence = bus.reset(&mut delay).map_err(|_| TemperatureSensorError::DiscoveryFailed)?;

        if presence {
            for _ in 0..3 {
                let device = bus.search_next(&mut DeviceSearch::new(), &mut delay)
                    .map_err(|_| TemperatureSensorError::DiscoveryFailed)?;
    
                if let Some(device) = device {
                    return DS18B20::new(device).map_err(|_| TemperatureSensorError::InitializationFailed);
                }
            }
    
            return Err(TemperatureSensorError::NotFound);
        }

        
        Err(TemperatureSensorError::NotFound)
    }
}

impl TemperatureSensorPort for Ds18b20TemperatureAdapter<'_> {
    async fn read_temperature(&mut self) -> Result<f64, TemperatureSensorError> {
        let mut delay = Delay::new();

        if self.sensor.is_none() {
            self.sensor = Some(Self::discover(&mut self.bus)?);
        }

        let sensor = self.sensor.as_ref().unwrap();

        let resolution = sensor.measure_temperature(&mut self.bus, &mut delay).map_err(|_| TemperatureSensorError::MeasurementStartFailed)?;

        Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

        let mut raw_temperature = None;

        for attempt in 0..3 {
            match sensor.read_temperature(&mut self.bus, &mut delay) {
                Ok(raw) => {
                    raw_temperature = Some(raw);
                    break;
                }
                Err(_) if attempt < 2 => Timer::after(Duration::from_millis(10)).await,
                Err(_) => return Err(TemperatureSensorError::MeasurementReadFailed),
            }
        }

        let raw_temperature = raw_temperature.ok_or(TemperatureSensorError::MeasurementReadFailed)?;

        Ok(raw_temperature as i16 as f64 / 16.0)
    }
}
