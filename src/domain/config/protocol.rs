use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains, TemperatureOptions, WeightOptions};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdateTemperatureOptions(TemperatureOptions),
    UpdatePidGains(PidGains),
    UpdateNetworkInfo(NetworkInfo),
    UpdateWeightOptions(WeightOptions),
    LogHeaterOptions,
    LogTemperatureOptions,
    LogPidGains,
    LogFirmwareInfo,
    LogNetworkInfo,
    LogWeightOptions,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum ConfigResponse {
    HeaterOptions(HeaterOptions),
    TemperatureOptions(TemperatureOptions),
    PidGains(PidGains),
    FirmwareInfo(FirmwareInfo),
    NetworkInfo(NetworkInfo),
    WeightOptions(WeightOptions),
}
