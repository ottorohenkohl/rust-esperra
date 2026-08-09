#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemperatureSensorCommand {
    Measure,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TemperatureSensorEvent {
    Measured(f64),
    MeasurementFailed,
}
