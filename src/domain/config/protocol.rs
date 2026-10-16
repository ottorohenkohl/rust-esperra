use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains, PreinfusionOptions, TemperatureOptions, WeightOptions};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdateTemperatureOptions(TemperatureOptions),
    UpdatePidGains(PidGains),
    UpdateNetworkInfo(NetworkInfo),
    UpdateWeightOptions(WeightOptions),
    UpdatePreinfusionOptions(PreinfusionOptions),
    LogHeaterOptions,
    LogTemperatureOptions,
    LogPidGains,
    LogFirmwareInfo,
    LogNetworkInfo,
    LogWeightOptions,
    LogPreinfusionOptions,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum ConfigResponse {
    HeaterOptions(HeaterOptions),
    TemperatureOptions(TemperatureOptions),
    PidGains(PidGains),
    FirmwareInfo(FirmwareInfo),
    NetworkInfo(NetworkInfo),
    WeightOptions(WeightOptions),
    PreinfusionOptions(PreinfusionOptions),
}
