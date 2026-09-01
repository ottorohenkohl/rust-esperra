use heapless::String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    StorageReadFailed,
    StorageWriteFailed,
}

pub trait ConfigPort {
    fn read_heater_options(&self) -> &HeaterOptions;
    fn read_pid_gains(&self) -> &PidGains;
    fn read_firmware_info(&self) -> &FirmwareInfo;
    fn read_network_info(&self) -> &NetworkInfo;
}

#[derive(Clone, Copy)]
pub struct HeaterOptions {
    pub heating_cycle_ms: u64,
    pub target_temperature: f64,
    pub target_temperature_interval: f64,
    pub boost_intervall_size: f64,
}

#[derive(Clone, Copy)]
pub struct PidGains {
    pub k_proportional: f64,
    pub k_intergral: f64,
    pub k_derivative: f64,
}

#[derive(Clone)]
pub struct FirmwareInfo {
    pub url: String<256>,
    pub version: String<32>,
}

#[derive(Clone)]
pub struct NetworkInfo {
    pub ntp_server: String<64>,
    pub wifi_ssid: String<32>,
    pub wifi_password: String<64>,
    pub mqtt_hostname: String<64>,
    pub mqtt_port: u16,
    pub mqtt_username: String<64>,
    pub mqtt_password: String<64>,
    pub mqtt_client_id: String<64>,
    pub mqtt_topic_prefix: String<64>,
}
