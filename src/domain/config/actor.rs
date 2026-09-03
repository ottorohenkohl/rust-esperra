use super::port::ConfigPort;
use super::protocol::ConfigMessage;

use crate::library::actor::interface::Actor;

use alloc::sync::Arc;

pub struct ConfigActor<C> {
    config_port: Arc<C>,
}

impl<C> ConfigActor<C> {
    pub fn new(config_port: Arc<C>) -> Self {
        Self { config_port }
    }
}

impl<C> Actor for ConfigActor<C>
where
    C: ConfigPort,
{
    type Message = ConfigMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            ConfigMessage::UpdateHeaterOptions(options) => self.config_port.write_heater_options(options).await,
            ConfigMessage::UpdateTemperatureOptions(options) => self.config_port.write_temperature_options(options).await,
            ConfigMessage::UpdatePidGains(gains) => self.config_port.write_pid_gains(gains).await,
            ConfigMessage::UpdateFirmwareInfo(info) => self.config_port.write_firmware_info(info).await,
            ConfigMessage::UpdateNetworkInfo(info) => self.config_port.write_network_info(info).await,
        }
    }
}
