use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use esperra_domain::actor::ActorRef;

pub struct EmbassyActorRef<'a, Message, const N: usize> {
    sender: Sender<'a, CriticalSectionRawMutex, Message, N>,
}

impl<'a, Message, const N: usize> EmbassyActorRef<'a, Message, N> {
    pub const fn new(sender: Sender<'a, CriticalSectionRawMutex, Message, N>) -> Self {
        Self { sender }
    }
}

impl<Message, const N: usize> ActorRef<Message> for EmbassyActorRef<'_, Message, N> {
    async fn send(&self, message: Message) {
        self.sender.send(message).await;
    }
}
