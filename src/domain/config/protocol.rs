use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains, TemperatureOptions};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdateTemperatureOptions(TemperatureOptions),
    UpdatePidGains(PidGains),
    UpdateFirmwareInfo(FirmwareInfo),
    UpdateNetworkInfo(NetworkInfo),
}
