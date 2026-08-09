use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Sender, TrySendError},
};

pub struct EmbassyActorRef<'a, Message, const N: usize> {
    sender: Sender<'a, CriticalSectionRawMutex, Message, N>,
}

impl<'a, Message, const N: usize> EmbassyActorRef<'a, Message, N> {
    pub const fn new(sender: Sender<'a, CriticalSectionRawMutex, Message, N>) -> Self {
        Self { sender }
    }

    pub fn try_send(&self, message: Message) -> Result<(), TrySendError<Message>> {
        self.sender.try_send(message)
    }
}
