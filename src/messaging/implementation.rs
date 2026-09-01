use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::NetworkInfo;
use crate::domain::update::protocol::UpdateMessage;
use crate::library::actor::ActorRef;
use crate::library::network::NetworkStack;

use heapless::String;

pub const TOPIC_CAPACITY: usize = 96;

pub struct MessagingService {
    pub network: &'static NetworkStack,
    pub hostname: String<64>,
    pub port: u16,
    pub client_id: String<64>,
    pub username: String<64>,
    pub password: String<64>,
    pub topic_prefix: String<64>,
    pub boiler: ActorRef<BoilerMessage>,
    pub brew: ActorRef<BrewMessage>,
    pub update: ActorRef<UpdateMessage>,
}

impl MessagingService {
    pub fn new( network: &'static NetworkStack, info: &NetworkInfo, boiler: ActorRef<BoilerMessage>, brew: ActorRef<BrewMessage>, update: ActorRef<UpdateMessage>) -> Self {
        let hostname = info.mqtt_hostname.clone();
        let port = info.mqtt_port;
        let client_id = info.mqtt_client_id.clone();
        let username = info.mqtt_username.clone();
        let password = info.mqtt_password.clone();
        let topic_prefix = info.mqtt_topic_prefix.clone();

        Self { network, hostname, port, client_id, username, password, topic_prefix, boiler, brew, update }
    }

    pub fn topic(&self, suffix: &str) -> Result<String<TOPIC_CAPACITY>, ()> {
        let mut topic = String::new();

        let prefix = self.topic_prefix.as_str();

        topic.push_str(prefix).map_err(|_| ())?;
        topic.push_str(suffix).map_err(|_| ())?;

        Ok(topic)
    }
}
