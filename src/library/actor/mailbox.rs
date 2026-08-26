use super::Actor;
use super::reference::ActorRef;

use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub(super) struct Envelope<Message> {
    pub(super) caller: Option<usize>,
    pub(super) message: Message,
}

pub struct Mailbox<Message, const SIZE: usize> {
    channel: Channel<CriticalSectionRawMutex, Envelope<Message>, SIZE>,
    lock: AtomicUsize,
}

impl<Message: 'static, const SIZE: usize> Mailbox<Message, SIZE> {
    pub const fn new() -> Self {
        let channel = Channel::new();

        Self { channel, lock: AtomicUsize::new(0) }
    }

    pub fn actor_ref(&'static self) -> ActorRef<Message> {
        let sender = self.channel.dyn_sender();
        let address = self as *const Self as usize;

        ActorRef::new(sender, address, &self.lock)
    }

    pub async fn run<A>(&'static self, mut actor: A) where A: Actor<Message = Message> {
        loop {
            let envelope = self.channel.receive().await;
            let owner = self.lock.load(Ordering::SeqCst);

            // Messages rejected by the lock at dispatch time are dropped, not deferred.
            if owner == 0 || envelope.caller == Some(owner) {
                actor.handle(envelope.message).await;
            }
        }
    }
}
