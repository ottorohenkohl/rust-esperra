use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TemperatureSensorError {
    DiscoveryFailed,
    NotFound,
    InitializationFailed,
    MeasurementStartFailed,
    MeasurementReadFailed,
}

pub trait InputPort {
    fn powered_state(&self) -> bool;
}

pub trait OutputPort {
    async fn set_active(&mut self, active: bool);
}

pub trait TemperatureSensorPort {
    async fn read_temperature(&mut self) -> Result<f64, TemperatureSensorError>;
}
