use super::port::{HeaterOptions, NetworkInfo, PidGains, PreinfusionOptions, ReadConfigPort, TemperatureOptions, WeightOptions, WriteConfigPort};
use super::protocol::{ConfigMessage, ConfigResponse};

use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;

use alloc::sync::Arc;

pub struct ConfigActor<C, L> {
    public_name: &'static str,

    config_port: Arc<C>,
    logging_port: Arc<L>,
}

impl<C, L> ConfigActor<C, L> {
    pub fn new(public_name: &'static str, config_port: Arc<C>, logging_port: Arc<L>) -> Self {
        Self { public_name, config_port, logging_port }
    }
}

impl<C, L> ConfigActor<C, L> where C: ReadConfigPort + WriteConfigPort, L: LoggingPort {
    async fn update_heater_options(&self, options: HeaterOptions) {
        self.config_port.write_heater_options(options).await;
    }

    async fn update_temperature_options(&self, options: TemperatureOptions) {
        self.config_port.write_temperature_options(options).await;
    }

    async fn update_pid_gains(&self, gains: PidGains) {
        self.config_port.write_pid_gains(gains).await;
    }


    async fn update_network_info(&self, info: NetworkInfo) {
        self.config_port.write_network_info(info).await;
    }

    async fn update_weight_options(&self, options: WeightOptions) {
        self.config_port.write_weight_options(options).await;
    }

    async fn update_preinfusion_options(&self, options: PreinfusionOptions) {
        self.config_port.write_preinfusion_options(options).await;
    }

    fn log_preinfusion_options(&self) {
        self.logging_port.log(self, Some("preinfusion_options"), &ConfigResponse::PreinfusionOptions(self.config_port.read_preinfusion_options()));
    }

    fn log_heater_options(&self) {
        self.logging_port.log(self, Some("heater_options"), &ConfigResponse::HeaterOptions(self.config_port.read_heater_options()));
    }

    fn log_temperature_options(&self) {
        self.logging_port.log(self, Some("temperature_options"), &ConfigResponse::TemperatureOptions(self.config_port.read_temperature_options()));
    }

    fn log_pid_gains(&self) {
        self.logging_port.log(self, Some("pid_gains"), &ConfigResponse::PidGains(self.config_port.read_pid_gains()));
    }

    fn log_firmware_info(&self) {
        self.logging_port.log(self, Some("firmware_info"), &ConfigResponse::FirmwareInfo(self.config_port.read_firmware_info()));
    }

    fn log_network_info(&self) {
        self.logging_port.log(self, Some("network_info"), &ConfigResponse::NetworkInfo(self.config_port.read_network_info()));
    }

    fn log_weight_options(&self) {
        self.logging_port.log(self, Some("weight_options"), &ConfigResponse::WeightOptions(self.config_port.read_weight_options()));
    }
}

impl<C, L> Actor for ConfigActor<C, L> where C: ReadConfigPort + WriteConfigPort, L: LoggingPort {
    type Message = ConfigMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            ConfigMessage::UpdateHeaterOptions(options) => self.update_heater_options(options).await,
            ConfigMessage::UpdateTemperatureOptions(options) => self.update_temperature_options(options).await,
            ConfigMessage::UpdatePidGains(gains) => self.update_pid_gains(gains).await,
            ConfigMessage::UpdateNetworkInfo(info) => self.update_network_info(info).await,
            ConfigMessage::UpdateWeightOptions(options) => self.update_weight_options(options).await,
            ConfigMessage::UpdatePreinfusionOptions(options) => self.update_preinfusion_options(options).await,

            ConfigMessage::LogHeaterOptions => self.log_heater_options(),
            ConfigMessage::LogTemperatureOptions => self.log_temperature_options(),
            ConfigMessage::LogPidGains => self.log_pid_gains(),
            ConfigMessage::LogFirmwareInfo => self.log_firmware_info(),
            ConfigMessage::LogNetworkInfo => self.log_network_info(),
            ConfigMessage::LogWeightOptions => self.log_weight_options(),
            ConfigMessage::LogPreinfusionOptions => self.log_preinfusion_options(),
        }
    }
}
