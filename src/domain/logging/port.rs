use serde::Serialize;

pub trait LoggingPort {
    fn log<Message>(&self, public_name: &str, message: &Message) where Message: Serialize;
}
