use super::port::TemperatureSensorError;

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightMessage {
    SetActive(bool),
    BlinkTimes(u8),
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureSensorMessage {
    MeasureTemperature,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum TemperatureSensorResponse {
    Measured(f64),
    MeasurementFailed(TemperatureSensorError),
}
