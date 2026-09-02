use super::port::{FirmwareInfo, HeaterOptions, NetworkInfo, PidGains};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ConfigMessage {
    UpdateHeaterOptions(HeaterOptions),
    UpdatePidGains(PidGains),
    UpdateFirmwareInfo(FirmwareInfo),
    UpdateNetworkInfo(NetworkInfo),
}
