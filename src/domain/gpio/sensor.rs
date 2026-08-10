#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TemperatureReading {
    Measured(f64),
    MeasurementFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureSensorMessage {
    MeasureTemperature,
}
