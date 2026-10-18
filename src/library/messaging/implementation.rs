use super::protocol::MessagingError;

use super::task::messaging_task;

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::cleaning::protocol::CleaningMessage;
use crate::domain::config::port::ReadConfigPort;
use crate::domain::config::protocol::ConfigMessage;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::update::protocol::UpdateMessage;
use crate::domain::weight::protocol::WeightMessage;
use crate::library::actor::model::ActorRef;
use crate::library::network::implementation::NetworkStackImpl;

use crate::adapter::logging::LoggedMessage;

use crate::adapter::logging;
use crate::domain::update::actor::update_in_progress;

use crate::library::network::implementation::ROOT_CA;

use alloc::sync::Arc;
use core::ffi::CStr;
use embassy_executor::Spawner;
use embassy_futures::select::Either;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer};
use embedded_io_async::{ErrorType, Read, Write};
use heapless::String;
use mbedtls_rs::{Certificate, ClientSessionConfig, Session, Split, SessionConfig};
use rust_mqtt::Bytes;
use rust_mqtt::buffer::AllocBuffer;
use rust_mqtt::client::event::Event;
use rust_mqtt::client::options::{ConnectOptions, PublicationOptions, SubscriptionOptions, TopicReference};
use rust_mqtt::client::Client;
use rust_mqtt::io::Transport;
use rust_mqtt::types::{MqttBinary, MqttString, TopicFilter, TopicName};
use serde::Deserialize;

use embassy_futures::select::select;
use embassy_time::with_timeout;
use serde_json_core::from_slice;

use mbedtls_rs::X509;

const SOCKET_BUFFER_SIZE: usize = 4096;

const NETWORK_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const TOPIC_CAPACITY: usize = 96;
const UPDATE_STATE_POLL_INTERVAL: Duration = Duration::from_millis(100);

struct Duplex<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> Duplex<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    fn split(&mut self) -> (&mut R, &mut W) {
        (&mut self.reader, &mut self.writer)
    }
}

impl<R: ErrorType, W: ErrorType<Error = R::Error>> ErrorType for Duplex<R, W> {
    type Error = R::Error;
}

impl<R: Read, W: ErrorType<Error = R::Error>> Read for Duplex<R, W> {
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.reader.read(buffer).await
    }
}

impl<R: ErrorType, W: Write<Error = R::Error>> Write for Duplex<R, W> {
    async fn write(&mut self, buffer: &[u8]) -> Result<usize, Self::Error> {
        self.writer.write(buffer).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.writer.flush().await
    }
}

impl<R: Read, W: Write<Error = R::Error>> Split for Duplex<R, W> {
    type Read<'a> = &'a mut R where Self: 'a;
    type Write<'a> = &'a mut W where Self: 'a;

    fn split(&mut self) -> (Self::Read<'_>, Self::Write<'_>) {
        Duplex::split(self)
    }
}

type MqttClient<'a, N> = Client<'a, N, AllocBuffer, 8, 1, 1, 0>;

enum ServeOutcome {
    UpdateRequested,
}

pub struct MqttMessagingImpl {
    network: &'static NetworkStackImpl,

    config_port: Arc<dyn ReadConfigPort>,

    boiler: ActorRef<BoilerMessage>,
    brew: ActorRef<BrewMessage>,
    update: ActorRef<UpdateMessage>,
    config: ActorRef<ConfigMessage>,
    energy: ActorRef<EnergyMessage>,
    weight: ActorRef<WeightMessage>,
    cleaning: ActorRef<CleaningMessage>,
}

impl MqttMessagingImpl {
    pub fn new(network: &'static NetworkStackImpl, config_port: Arc<dyn ReadConfigPort>, boiler: ActorRef<BoilerMessage>, brew: ActorRef<BrewMessage>, update: ActorRef<UpdateMessage>, config: ActorRef<ConfigMessage>, energy: ActorRef<EnergyMessage>, weight: ActorRef<WeightMessage>, cleaning: ActorRef<CleaningMessage>, spawner: Spawner) {
        let implementation = Self { network, config_port, boiler, brew, update, config, energy, weight, cleaning };

        spawner.spawn(messaging_task(implementation).unwrap());
    }

    fn topic(topic_prefix: &str, suffix: &str) -> Result<String<TOPIC_CAPACITY>, MessagingError> {
        let mut topic = String::new();

        topic.push_str(topic_prefix).map_err(|_| MessagingError::Config)?;
        topic.push_str(suffix).map_err(|_| MessagingError::Config)?;

        Ok(topic)
    }
}

impl MqttMessagingImpl {
    pub async fn run(self) -> ! {
        loop {
            match self.connect_and_serve().await {
                Ok(ServeOutcome::UpdateRequested) => self.wait_for_update().await,
                Err(_) => {
                    Timer::after(RECONNECT_DELAY).await;
                }
            }
        }
    }

    async fn wait_for_update(&self) {
        Timer::after(UPDATE_STATE_POLL_INTERVAL).await;

        while update_in_progress() {
            Timer::after(UPDATE_STATE_POLL_INTERVAL).await;
        }
    }

    async fn connect_and_serve(&self) -> Result<ServeOutcome, MessagingError> {
        let info = self.config_port.read_network_info();

        self.network.stack.wait_config_up().await;

        with_timeout(NETWORK_OPERATION_TIMEOUT, self.network.synchronize_time(info.ntp_server.as_str())).await.map_err(|_| MessagingError::Mqtt)?.map_err(|_| MessagingError::Mqtt)?;

        let addresses = self.network.stack.dns_query(info.mqtt_hostname.as_str(), DnsQueryType::A).await.map_err(|_| MessagingError::Dns)?;

        let mut rx_buffer = [0; SOCKET_BUFFER_SIZE];
        let mut tx_buffer = [0; SOCKET_BUFFER_SIZE];

        let mut socket = TcpSocket::new(self.network.stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(NETWORK_OPERATION_TIMEOUT));
        socket.connect((*addresses.first().ok_or(MessagingError::Dns)?, info.mqtt_port)).await.map_err(|_| MessagingError::Tcp)?;

        let mut hostname = [0u8; 65];

        let hostname_bytes = info.mqtt_hostname.as_bytes();

        hostname[..hostname_bytes.len()].copy_from_slice(hostname_bytes);

        let server_name = CStr::from_bytes_with_nul(&hostname[..hostname_bytes.len() + 1]).map_err(|_| MessagingError::Config)?;
        let root_ca = CStr::from_bytes_with_nul(ROOT_CA).map_err(|_| MessagingError::Tls)?;
        let ca_chain = Certificate::new(X509::PEM(root_ca)).map_err(|_| MessagingError::Tls)?;

        let client_session_config = ClientSessionConfig { ca_chain: Some(ca_chain), server_name: Some(server_name), ..ClientSessionConfig::new() };
        let tls_config = SessionConfig::Client(client_session_config);

        let (tcp_reader, tcp_writer) = socket.split();

        let mut tls = Session::new(self.network.tls.reference(), Duplex::new(tcp_reader, tcp_writer), &tls_config).map_err(|_| MessagingError::Tls)?;

        with_timeout(NETWORK_OPERATION_TIMEOUT, tls.connect()).await.map_err(|_| MessagingError::Tls)?.map_err(|_| MessagingError::Tls)?;

        let (tls_reader, tls_writer) = tls.split().await.map_err(|_| MessagingError::Tls)?;

        let mut buffer = AllocBuffer;
        let mut client = MqttClient::new(&mut buffer);
        let mut options = ConnectOptions::new().clean_start();

        options = options.user_name(MqttString::from_str(info.mqtt_username.as_str()).map_err(|_| MessagingError::Config)?);
        options = options.password(MqttBinary::from_slice(info.mqtt_password.as_bytes()).map_err(|_| MessagingError::Config)?);

        with_timeout(NETWORK_OPERATION_TIMEOUT, client.connect(Duplex::new(tls_reader, tls_writer), &options, Some(MqttString::from_str(info.mqtt_client_id.as_str()).map_err(|_| MessagingError::Config)?))).await.map_err(|_| MessagingError::Mqtt)?.map_err(|_| MessagingError::Mqtt)?;

        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/boiler").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/brew").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/update").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/config").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/energy").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/weight").await?;
        self.subscribe(&mut client, info.mqtt_topic_prefix.as_str(), "/in/cleaning").await?;

        loop {
            match select(client.poll_header(), logging::receive_log()).await {
                Either::Second(message) => self.publish_log(&mut client, info.mqtt_topic_prefix.as_str(), message).await?,
                Either::First(header) => {
                    let event = with_timeout(NETWORK_OPERATION_TIMEOUT, client.poll_body(header.map_err(|_| MessagingError::Mqtt)?)).await.map_err(|_| MessagingError::Mqtt)?.map_err(|_| MessagingError::Mqtt)?;

                    if let Event::Publish(publish) = event {
                        let topic = publish.topic.as_ref().as_ref();
                        let payload = publish.message.as_ref();

                        if self.handle_command(info.mqtt_topic_prefix.as_str(), topic, payload)? {
                            return Ok(ServeOutcome::UpdateRequested);
                        }
                    }
                }
            }
        }
    }

    async fn publish_log<N: Transport>(&self, client: &mut MqttClient<'_, N>, topic_prefix: &str, message: LoggedMessage) -> Result<(), MessagingError> {
        let mut suffix: String<TOPIC_CAPACITY> = String::new();

        suffix.push_str("/out/").map_err(|_| MessagingError::Config)?;
        suffix.push_str(message.public_name.as_str()).map_err(|_| MessagingError::Config)?;

        let topic = Self::topic(topic_prefix, suffix.as_str())?;
        let topic = TopicName::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?;
        let options = PublicationOptions::new(TopicReference::Name(topic)).payload_format_indicator(true);

        client.publish(&options, Bytes::from(message.payload.as_slice())).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }

    async fn subscribe<N: Transport>(&self, client: &mut MqttClient<'_, N>, topic_prefix: &str, suffix: &str) -> Result<(), MessagingError> {
        let topic = Self::topic(topic_prefix, suffix).map_err(|_| MessagingError::Config)?;

        client.subscribe(TopicFilter::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?, SubscriptionOptions::new()).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }

    fn dispatch<Message>(&self, payload: &[u8], actor: &ActorRef<Message>) -> bool where Message: for<'a> Deserialize<'a> + 'static {
        match from_slice(payload) {
            Ok((command, _)) => {
                actor.tell(None, command);

                true
            }
            Err(_) => false,
        }
    }

    fn handle_command(&self, topic_prefix: &str, topic: &str, payload: &[u8]) -> Result<bool, MessagingError> {
        let boiler_topic = Self::topic(topic_prefix, "/in/boiler").map_err(|_| MessagingError::Config)?;
        let brew_topic = Self::topic(topic_prefix, "/in/brew").map_err(|_| MessagingError::Config)?;
        let update_topic = Self::topic(topic_prefix, "/in/update").map_err(|_| MessagingError::Config)?;
        let config_topic = Self::topic(topic_prefix, "/in/config").map_err(|_| MessagingError::Config)?;
        let energy_topic = Self::topic(topic_prefix, "/in/energy").map_err(|_| MessagingError::Config)?;
        let weight_topic = Self::topic(topic_prefix, "/in/weight").map_err(|_| MessagingError::Config)?;

        let cleaning_topic = Self::topic(topic_prefix, "/in/cleaning").map_err(|_| MessagingError::Config)?;

        match topic {
            topic if topic == cleaning_topic.as_str() => {
                self.dispatch(payload, &self.cleaning);
            }
            topic if topic == boiler_topic.as_str() => {
                self.dispatch(payload, &self.boiler);
            }
            topic if topic == brew_topic.as_str() => {
                self.dispatch(payload, &self.brew);
            }
            topic if topic == update_topic.as_str() => {
                return Ok(self.dispatch(payload, &self.update));
            }
            topic if topic == config_topic.as_str() => {
                self.dispatch(payload, &self.config);
            }
            topic if topic == energy_topic.as_str() => {
                self.dispatch(payload, &self.energy);
            }
            topic if topic == weight_topic.as_str() => {
                self.dispatch(payload, &self.weight);
            }

            _ => {}
        }

        Ok(false)
    }
}
