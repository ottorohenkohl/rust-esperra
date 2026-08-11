use super::Actor;
use super::reference::ActorRef;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub struct Mailbox<Message> {
    channel: Channel<CriticalSectionRawMutex, Message, 8>,
}

impl<Message: 'static> Mailbox<Message> {
    pub const fn new() -> Self {
        let channel = Channel::new();

        Self { channel }
    }

    pub fn actor_ref(&'static self) -> ActorRef<Message> {
        let sender = self.channel.dyn_sender();

        ActorRef::new(sender)
    }

    pub async fn run<A>(&'static self, mut actor: A) where A: Actor<Message = Message> {
        loop {
            let message = self.channel.receive().await;

            actor.handle(message).await;
        }
    }
}
