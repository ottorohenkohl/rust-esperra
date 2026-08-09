use crate::domain::{
    actor::{ActorRef, MailboxError},
    boiler::BoilerMessage,
    output::OutputMessage,
    sensor::TemperatureSensorMessage,
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};

pub struct EmbassyActorRef<'a, Message, const N: usize> {
    sender: Sender<'a, CriticalSectionRawMutex, Message, N>,
}

impl<'a, Message, const N: usize> EmbassyActorRef<'a, Message, N> {
    pub const fn new(sender: Sender<'a, CriticalSectionRawMutex, Message, N>) -> Self {
        Self { sender }
    }
}

impl<Message, const N: usize> ActorRef<Message> for EmbassyActorRef<'_, Message, N> {
    fn try_send(&self, message: Message) -> Result<(), MailboxError> {
        self.sender
            .try_send(message)
            .map_err(|_| MailboxError::Full)
    }
}

pub type BoilerActorRef<'a> = EmbassyActorRef<'a, BoilerMessage, 4>;
pub type OutputActorRef<'a> = EmbassyActorRef<'a, OutputMessage, 1>;
pub type TemperatureSensorActorRef<'a> = EmbassyActorRef<'a, TemperatureSensorMessage, 1>;
