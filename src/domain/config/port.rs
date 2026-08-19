pub trait ConfigPort {
    fn read_heater_options(&self) -> &HeaterOptions;
    fn read_pid_gains(&self) -> &PidGains;
    fn read_firmware_info(&self) -> &FirmwareInfo;
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
    pub url: heapless::String<256>,
    pub old_version: heapless::String<32>,
}
