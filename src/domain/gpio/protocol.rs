#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMessage {
    SetActive(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightMessage {
    SetActive(bool),
    BlinkOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureSensorMessage {
    MeasureTemperature,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TemperatureSensorResponse {
    Measured(f64),
    MeasurementFailed,
}
