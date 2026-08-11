use crate::library::error::Error;

pub trait InputPort {
    fn powered_state(&mut self) -> bool;
}

pub trait LightPort {
    async fn set_active(&mut self, active: bool) -> Result<(), Error>;
    async fn blink(&mut self, times: u8) -> Result<(), Error>;
}

pub trait TemperatureSensorPort {
    async fn read_temperature(&mut self) -> Result<f64, Error>;
}
