use crate::library::actor::interface::Actor;

use serde::Serialize;

pub trait LoggingPort {
    fn log<A, Message>(&self, actor: &A, subtopic: Option<&str>, message: &Message) where A: Actor, Message: Serialize;
}
