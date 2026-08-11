use crate::domain::gpio::protocol::TemperatureSensorResponse;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerMessage {
    UpdateTemperature { measured_temperature: f64 },
    StopHeating,
    SwitchHeater,
}

impl From<TemperatureSensorResponse> for BoilerMessage {
    fn from(reading: TemperatureSensorResponse) -> Self {
        match reading {
            TemperatureSensorResponse::Measured(measured_temperature) => Self::UpdateTemperature { measured_temperature },
            TemperatureSensorResponse::MeasurementFailed => Self::StopHeating,
        }
    }
}
