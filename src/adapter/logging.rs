use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::{String, Vec};
use serde::Serialize;

const LOG_MESSAGE_CAPACITY: usize = 1024;
const PUBLIC_NAME_CAPACITY: usize = 64;
const OUTBOUND_LOG_CAPACITY: usize = 16;

static OUTBOUND_LOGS: Channel<CriticalSectionRawMutex, LoggedMessage, OUTBOUND_LOG_CAPACITY> = Channel::new();

pub struct MqttLoggingAdapter;

pub struct LoggedMessage {
    pub public_name: String<PUBLIC_NAME_CAPACITY>,
    pub payload: Vec<u8, LOG_MESSAGE_CAPACITY>,
}

impl MqttLoggingAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl LoggingPort for MqttLoggingAdapter {
    fn log<A, Message>(&self, actor: &A, subtopic: Option<&str>, message: &Message) where A: Actor, Message: Serialize {
        let Ok(mut public_name) = String::try_from(actor.name()) else { return };

        if let Some(subtopic) = subtopic {
            if public_name.push('/').is_err() || public_name.push_str(subtopic).is_err() {
                return;
            }
        }

        let mut buffer = [0; LOG_MESSAGE_CAPACITY];

        let Ok(length) = serde_json_core::to_slice(message, &mut buffer) else { return };
        let Ok(payload) = Vec::from_slice(&buffer[..length]) else { return };

        let _ = OUTBOUND_LOGS.try_send(LoggedMessage { public_name, payload });
    }
}

pub async fn receive_log() -> LoggedMessage {
    OUTBOUND_LOGS.receive().await
}
