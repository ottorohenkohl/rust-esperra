use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains, TemperatureOptions};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdateTemperatureOptions(TemperatureOptions),
    UpdatePidGains(PidGains),
    UpdateFirmwareInfo(FirmwareInfo),
    UpdateNetworkInfo(NetworkInfo),
    LogHeaterOptions,
    LogTemperatureOptions,
    LogPidGains,
    LogFirmwareInfo,
    LogNetworkInfo,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum ConfigResponse {
    HeaterOptions(HeaterOptions),
    TemperatureOptions(TemperatureOptions),
    PidGains(PidGains),
    FirmwareInfo(FirmwareInfo),
    NetworkInfo(NetworkInfo),
}
