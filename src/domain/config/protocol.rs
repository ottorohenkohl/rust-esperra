use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains, TemperatureOptions};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdateTemperatureOptions(TemperatureOptions),
    UpdatePidGains(PidGains),
    UpdateFirmwareInfo(FirmwareInfo),
    UpdateNetworkInfo(NetworkInfo),
}
