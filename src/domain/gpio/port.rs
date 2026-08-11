use crate::library::error::Error;

pub trait InputPort {
    fn powered_state(&mut self) -> bool;
}

pub trait OutputPort {
    async fn invert_state(&mut self);
    async fn set_active(&mut self, active: bool);
}

pub trait TemperatureSensorPort {
    async fn read_temperature(&mut self) -> Result<f64, Error>;
}

pub trait TimerPort {
    async fn wait_for(&self, millis: u64);
}
