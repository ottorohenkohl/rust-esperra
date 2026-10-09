use super::port::TemperatureSensorError;

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightMode {
    Off,
    On,
    Blinking,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingMessage {
    SetPower(LightMode),
    SetStatus(LightMode),
    SetBoth(LightMode, LightMode),
    BlinkStatusTimes(u8),
    Tick,
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
