use super::port::{ReadConfigPort, WriteConfigPort};
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

impl<C, L> Actor for ConfigActor<C, L>
where
    C: ReadConfigPort + WriteConfigPort,
    L: LoggingPort,
{
    type Message = ConfigMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            ConfigMessage::UpdateHeaterOptions(options) => self.config_port.write_heater_options(options).await,
            ConfigMessage::UpdateTemperatureOptions(options) => self.config_port.write_temperature_options(options).await,
            ConfigMessage::UpdatePidGains(gains) => self.config_port.write_pid_gains(gains).await,
            ConfigMessage::UpdateFirmwareInfo(info) => self.config_port.write_firmware_info(info).await,
            ConfigMessage::UpdateNetworkInfo(info) => self.config_port.write_network_info(info).await,

            ConfigMessage::LogHeaterOptions => self.logging_port.log(self, Some("heater_options"), &ConfigResponse::HeaterOptions(self.config_port.read_heater_options())),
            ConfigMessage::LogTemperatureOptions => self.logging_port.log(self, Some("temperature_options"), &ConfigResponse::TemperatureOptions(self.config_port.read_temperature_options())),
            ConfigMessage::LogPidGains => self.logging_port.log(self, Some("pid_gains"), &ConfigResponse::PidGains(self.config_port.read_pid_gains())),
            ConfigMessage::LogFirmwareInfo => self.logging_port.log(self, Some("firmware_info"), &ConfigResponse::FirmwareInfo(self.config_port.read_firmware_info())),
            ConfigMessage::LogNetworkInfo => self.logging_port.log(self, Some("network_info"), &ConfigResponse::NetworkInfo(self.config_port.read_network_info())),
        }
    }
}
