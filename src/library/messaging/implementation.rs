use crate::adapter::logging::{self, LoggedMessage};
use super::protocol::MessagingError;
use super::task::messaging_task;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::NetworkInfo;
use crate::domain::config::protocol::ConfigMessage;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::model::ActorRef;
use crate::library::network::implementation::NetworkStackImpl;

use crate::library::network::implementation::ROOT_CA;

use core::ffi::CStr;
use embassy_executor::Spawner;
use embassy_futures::select::Either;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_time::{with_timeout, Duration, Timer};
use heapless::String;
use mbedtls_rs::{Certificate, ClientSessionConfig, Session, SessionConfig};
use rust_mqtt::Bytes;
use rust_mqtt::buffer::AllocBuffer;
use rust_mqtt::client::event::Event;
use rust_mqtt::client::options::{ConnectOptions, PublicationOptions, SubscriptionOptions, TopicReference};
use rust_mqtt::client::Client;
use rust_mqtt::types::{MqttBinary, MqttString, TopicFilter, TopicName};
use serde::Deserialize;
use serde_json_core::from_slice;

use embassy_futures::select::select;

use mbedtls_rs::X509;

const SOCKET_BUFFER_SIZE: usize = 4096;

const NETWORK_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const TOPIC_CAPACITY: usize = 96;

type MqttClient<'a> = Client<'a, Session<'a, TcpSocket<'a>>, AllocBuffer, 5, 1, 1, 0>;

pub struct MqttMessagingImpl {
    network: &'static NetworkStackImpl,
    hostname: String<64>,
    port: u16,
    client_id: String<64>,
    username: String<64>,
    password: String<64>,
    ntp_server: String<64>,
    topic_prefix: String<64>,
    boiler: ActorRef<BoilerMessage>,
    brew: ActorRef<BrewMessage>,
    update: ActorRef<UpdateMessage>,
    config: ActorRef<ConfigMessage>,
    energy: ActorRef<EnergyMessage>,
}

impl MqttMessagingImpl {
    pub fn new(network: &'static NetworkStackImpl, info: &NetworkInfo, boiler: ActorRef<BoilerMessage>, brew: ActorRef<BrewMessage>, update: ActorRef<UpdateMessage>, config: ActorRef<ConfigMessage>, energy: ActorRef<EnergyMessage>, spawner: Spawner) {
        let implementation = Self { network, hostname: info.mqtt_hostname.clone(), port: info.mqtt_port, client_id: info.mqtt_client_id.clone(), username: info.mqtt_username.clone(), password: info.mqtt_password.clone(), ntp_server: info.ntp_server.clone(), topic_prefix: info.mqtt_topic_prefix.clone(), boiler, brew, update, config, energy };

        spawner.spawn(messaging_task(implementation).unwrap());
    }

    fn topic(&self, suffix: &str) -> Result<String<TOPIC_CAPACITY>, MessagingError> {
        let mut topic = String::new();

        topic.push_str(self.topic_prefix.as_str()).map_err(|_| MessagingError::Config)?;
        topic.push_str(suffix).map_err(|_| MessagingError::Config)?;

        Ok(topic)
    }
}

impl MqttMessagingImpl {
    pub async fn run(self) -> ! {
        loop {
            if let Err(_) = self.connect_and_serve().await {
                Timer::after(RECONNECT_DELAY).await;
            }
        }
    }

    async fn connect_and_serve(&self) -> Result<(), MessagingError> {
        if self.hostname.is_empty() || self.client_id.is_empty() || self.topic_prefix.is_empty() {
            return Err(MessagingError::Config);
        }

        self.network.stack.wait_config_up().await;

        with_timeout(NETWORK_OPERATION_TIMEOUT, self.network.synchronize_time(self.ntp_server.as_str())).await.map_err(|_| MessagingError::Mqtt)?.map_err(|_| MessagingError::Mqtt)?;

        let addresses = self.network.stack.dns_query(self.hostname.as_str(), DnsQueryType::A).await.map_err(|_| MessagingError::Dns)?;

        let mut rx_buffer = [0; SOCKET_BUFFER_SIZE];
        let mut tx_buffer = [0; SOCKET_BUFFER_SIZE];

        let mut socket = TcpSocket::new(self.network.stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(NETWORK_OPERATION_TIMEOUT));
        socket.connect((*addresses.first().ok_or(MessagingError::Dns)?, self.port)).await.map_err(|_| MessagingError::Tcp)?;

        let mut hostname = [0u8; 65];

        let hostname_bytes = self.hostname.as_bytes();

        hostname[..hostname_bytes.len()].copy_from_slice(hostname_bytes);

        let server_name = CStr::from_bytes_with_nul(&hostname[..hostname_bytes.len() + 1]).map_err(|_| MessagingError::Config)?;
        let root_ca = CStr::from_bytes_with_nul(ROOT_CA).map_err(|_| MessagingError::Tls)?;
        let ca_chain = Certificate::new(X509::PEM(root_ca)).map_err(|_| MessagingError::Tls)?;

        let client_session_config = ClientSessionConfig { ca_chain: Some(ca_chain), server_name: Some(server_name), ..ClientSessionConfig::new() };
        let tls_config = SessionConfig::Client(client_session_config);

        let mut tls = Session::new(self.network.tls.reference(), socket, &tls_config).map_err(|_| MessagingError::Tls)?;

        with_timeout(NETWORK_OPERATION_TIMEOUT, tls.connect()).await.map_err(|_| MessagingError::Tls)?.map_err(|_| MessagingError::Tls)?;

        let mut buffer = AllocBuffer;
        let mut client = MqttClient::new(&mut buffer);
        let mut options = ConnectOptions::new().clean_start();

        if !self.username.is_empty() {
            options = options.user_name(MqttString::from_str(self.username.as_str()).map_err(|_| MessagingError::Config)?);
        }

        if !self.password.is_empty() {
            options = options.password(MqttBinary::from_slice(self.password.as_bytes()).map_err(|_| MessagingError::Config)?);
        }

        with_timeout(NETWORK_OPERATION_TIMEOUT, client.connect(tls, &options, Some(MqttString::from_str(self.client_id.as_str()).map_err(|_| MessagingError::Config)?))).await.map_err(|_| MessagingError::Mqtt)?.map_err(|_| MessagingError::Mqtt)?;

        self.subscribe(&mut client, "/in/boiler").await?;
        self.subscribe(&mut client, "/in/brew").await?;
        self.subscribe(&mut client, "/in/update").await?;
        self.subscribe(&mut client, "/in/config").await?;
        self.subscribe(&mut client, "/in/energy").await?;


        loop {
            match select(logging::receive_log(), client.poll()).await {
                Either::First(message) => self.publish_log(&mut client, message).await?,
                Either::Second(event) => {
                    if let Event::Publish(publish) = event.map_err(|_| MessagingError::Mqtt)? {
                        let topic = publish.topic.as_ref().as_ref();
                        let payload = publish.message.as_ref();

                        self.handle_command(topic, payload)?;
                    }
                }
            }
        }
    }

    async fn publish_log(&self, client: &mut MqttClient<'_>, message: LoggedMessage) -> Result<(), MessagingError> {
        let mut suffix: String<TOPIC_CAPACITY> = String::new();

        suffix.push_str("/out/").map_err(|_| MessagingError::Config)?;

        suffix.push_str(message.public_name.as_str()).map_err(|_| MessagingError::Config)?;

        let topic = self.topic(suffix.as_str())?;

        let topic = TopicName::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?;
        let options = PublicationOptions::new(TopicReference::Name(topic)).payload_format_indicator(true);

        client.publish(&options, Bytes::from(message.payload.as_slice())).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }

    async fn subscribe(&self, client: &mut MqttClient<'_>, suffix: &str) -> Result<(), MessagingError> {
        let topic = self.topic(suffix).map_err(|_| MessagingError::Config)?;

        client.subscribe(TopicFilter::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?, SubscriptionOptions::new()).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }

    fn dispatch<Message>(&self, payload: &[u8], actor: &ActorRef<Message>) where Message: for<'a> Deserialize<'a> + 'static {
        if let Ok((command, _)) = from_slice(payload) {
            actor.tell(None, command);
        }
    }

    fn handle_command(&self, topic: &str, payload: &[u8]) -> Result<(), MessagingError> {
        let boiler_topic = self.topic("/in/boiler").map_err(|_| MessagingError::Config)?;
        let brew_topic = self.topic("/in/brew").map_err(|_| MessagingError::Config)?;
        let update_topic = self.topic("/in/update").map_err(|_| MessagingError::Config)?;
        let config_topic = self.topic("/in/config").map_err(|_| MessagingError::Config)?;
        let energy_topic = self.topic("/in/energy").map_err(|_| MessagingError::Config)?;

        match topic {
            topic if topic == boiler_topic.as_str() => self.dispatch(payload, &self.boiler),
            topic if topic == brew_topic.as_str() => self.dispatch(payload, &self.brew),
            topic if topic == update_topic.as_str() => self.dispatch(payload, &self.update),
            topic if topic == config_topic.as_str() => self.dispatch(payload, &self.config),
            topic if topic == energy_topic.as_str() => self.dispatch(payload, &self.energy),

            _ => {}
        }

        Ok(())
    }
}
