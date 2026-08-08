pub trait ConfigProvider {
    fn read_heater_options(&self) -> &HeaterOptions;
    fn read_pid_gains(&self) -> &PidGains;
}

pub struct HeaterOptions {
    pub heating_cycle_ms: u64,
    pub target_temperature: f64,
    pub boost_intervall_size: f64,
}



pub struct PidGains {
    pub k_proportional: f64,
    pub k_intergral: f64,
    pub k_derivative: f64,
}
