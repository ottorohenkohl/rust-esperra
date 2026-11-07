use crate::domain::gpio::protocol::TemperatureSensorResponse;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum CleaningMessage {
    Start,
    NotEnoughWater,
    Tick,
    UpdateTemperature(Option<f64>),
}

impl From<TemperatureSensorResponse> for CleaningMessage {
    fn from(reading: TemperatureSensorResponse) -> Self {
        match reading {
            TemperatureSensorResponse::Measured(value) => Self::UpdateTemperature(Some(value)),
            TemperatureSensorResponse::MeasurementFailed(_) => Self::UpdateTemperature(None),
        }
    }
}
