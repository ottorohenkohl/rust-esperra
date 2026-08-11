use crate::library::error::Error;

use embassy_time::Duration;

pub trait InputPort {
    fn powered_state(&mut self) -> bool;
}

pub trait LightPort {
    async fn set_active(&mut self, active: bool) -> Result<(), Error>;
    async fn blink(&mut self, duration: Duration) -> Result<(), Error>;
}

pub trait TemperatureSensorPort {
    async fn read_temperature(&mut self) -> Result<f64, Error>;
}
