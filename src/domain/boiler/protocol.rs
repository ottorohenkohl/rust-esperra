use crate::domain::gpio::protocol::TemperatureSensorResponse;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerMessage {
    RegulateHeating { measured_temperature: f64 },
    StopHeating,
}

impl From<TemperatureSensorResponse> for BoilerMessage {
    fn from(reading: TemperatureSensorResponse) -> Self {
        match reading {
            TemperatureSensorResponse::Measured(measured_temperature) => Self::RegulateHeating { measured_temperature },
            TemperatureSensorResponse::MeasurementFailed => Self::StopHeating,
        }
    }
}
