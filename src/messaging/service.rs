use super::implementation::MessagingService;
use super::interface::{MessagingError, MessagingEvent};

use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::update::protocol::UpdateMessage;

use crate::library::network::ROOT_CA;

use core::ffi::CStr;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer};
use mbedtls_rs::{Certificate, ClientSessionConfig, Session, SessionConfig, X509};
use rust_mqtt::Bytes;
use rust_mqtt::buffer::AllocBuffer;
use rust_mqtt::client::event::Event;
use rust_mqtt::client::options::{ConnectOptions, PublicationOptions, SubscriptionOptions, TopicReference};
use rust_mqtt::client::Client;
use rust_mqtt::types::{MqttBinary, MqttString, TopicFilter, TopicName};

const SOCKET_BUFFER_SIZE: usize = 4096;
const JSON_BUFFER_SIZE: usize = 256;
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

type MqttClient<'a> = Client<'a, Session<'a, TcpSocket<'a>>, AllocBuffer, 3, 1, 1, 0>;

impl MessagingService {
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

        let addresses = self.network.stack.dns_query(self.hostname.as_str(), DnsQueryType::A).await.map_err(|_| MessagingError::Dns)?;

        let mut rx_buffer = [0; SOCKET_BUFFER_SIZE];
        let mut tx_buffer = [0; SOCKET_BUFFER_SIZE];

        let mut socket = TcpSocket::new(self.network.stack, &mut rx_buffer, &mut tx_buffer);

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

        tls.connect().await.map_err(|_| MessagingError::Tls)?;

        let mut buffer = AllocBuffer;
        let mut client = MqttClient::new(&mut buffer);
        let mut options = ConnectOptions::new().clean_start();

        if !self.username.is_empty() {
            options = options.user_name(MqttString::from_str(self.username.as_str()).map_err(|_| MessagingError::Config)?);
        }

        if !self.password.is_empty() {
            options = options.password(MqttBinary::from_slice(self.password.as_bytes()).map_err(|_| MessagingError::Config)?);
        }

        client.connect(tls, &options, Some(MqttString::from_str(self.client_id.as_str()).map_err(|_| MessagingError::Config)?)).await.map_err(|_| MessagingError::Mqtt)?;

        self.subscribe(&mut client, "/in/boiler").await?;
        self.subscribe(&mut client, "/in/brew").await?;
        self.subscribe(&mut client, "/in/update").await?;

        self.publish_event(&mut client, "/out/system", MessagingEvent::Connected).await?;

        loop {
            match client.poll().await.map_err(|_| MessagingError::Mqtt)? {
                Event::Publish(publish) => {
                    let topic = publish.topic.as_ref().as_ref();
                    let payload = publish.message.as_ref();

                    self.handle_command(&mut client, topic, payload).await?;
                }
                _ => {}
            }
        }
    }

    async fn subscribe(&self, client: &mut MqttClient<'_>, suffix: &str) -> Result<(), MessagingError> {
        let topic = self.topic(suffix).map_err(|_| MessagingError::Config)?;

        client.subscribe(TopicFilter::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?, SubscriptionOptions::new()).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }

    async fn handle_command(&self, client: &mut MqttClient<'_>, topic: &str, payload: &[u8]) -> Result<(), MessagingError> {
        let boiler_topic = self.topic("/in/boiler").map_err(|_| MessagingError::Config)?;
        let brew_topic = self.topic("/in/brew").map_err(|_| MessagingError::Config)?;
        let update_topic = self.topic("/in/update").map_err(|_| MessagingError::Config)?;

        if topic == boiler_topic.as_str() {
            return self.dispatch(client, "/out/boiler", serde_json_core::from_slice::<BoilerMessage>(payload).map(|(command, _)| self.boiler.tell(None, command))).await;
        }

        if topic == brew_topic.as_str() {
            return self.dispatch(client, "/out/brew", serde_json_core::from_slice::<BrewMessage>(payload).map(|(command, _)| self.brew.tell(None, command))).await;
        }

        if topic == update_topic.as_str() {
            return self.dispatch(client, "/out/update", serde_json_core::from_slice::<UpdateMessage>(payload).map(|(command, _)| self.update.tell(None, command))).await;
        }

        Ok(())
    }

    async fn dispatch<E>(&self, client: &mut MqttClient<'_>, topic: &str, result: Result<(), E>) -> Result<(), MessagingError> {
        let event = if result.is_ok() { MessagingEvent::CommandAccepted } else { MessagingEvent::BadFormat };

        self.publish_event(client, topic, event).await
    }

    async fn publish_event(&self, client: &mut MqttClient<'_>, suffix: &str, event: MessagingEvent) -> Result<(), MessagingError> {
        let topic = self.topic(suffix).map_err(|_| MessagingError::Config)?;

        let topic = TopicName::new(MqttString::from_str(topic.as_str()).map_err(|_| MessagingError::Config)?).ok_or(MessagingError::Config)?;

        let mut payload = [0; JSON_BUFFER_SIZE];

        let payload_length = serde_json_core::to_slice(&event, &mut payload).map_err(|_| MessagingError::Mqtt)?;

        let options = PublicationOptions::new(TopicReference::Name(topic)).payload_format_indicator(true);

        client.publish(&options, Bytes::from(&payload[..payload_length])).await.map_err(|_| MessagingError::Mqtt)?;

        Ok(())
    }
}
