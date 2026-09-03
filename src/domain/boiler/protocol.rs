use crate::domain::gpio::protocol::TemperatureSensorResponse;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum BoilerMessage {
    SetActive(bool),
    SwitchHeater,
    UpdateTemperature(f64),
}

impl From<TemperatureSensorResponse> for BoilerMessage {
    fn from(reading: TemperatureSensorResponse) -> Self {
        match reading {
            TemperatureSensorResponse::Measured(measured_temperature) => Self::UpdateTemperature(measured_temperature),
            TemperatureSensorResponse::MeasurementFailed(_) => Self::SetActive(false),
        }
    }
}
