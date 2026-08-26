use super::Actor;
use super::reference::ActorRef;

use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use core::ptr::from_ref;

pub struct Envelope<Message> {
    pub caller: Option<usize>,
    pub message: Message,
}

pub trait Mailbox<Message: 'static> {
    fn address(&self) -> usize;
    fn try_send(&self, envelope: Envelope<Message>);
    fn lock(&self, owner: usize) -> bool;
    fn unlock(&self, owner: usize) -> bool;
}

pub struct EmbassyMailbox<Message, const SIZE: usize> {
    channel: Channel<CriticalSectionRawMutex, Envelope<Message>, SIZE>,
    lock: AtomicUsize,
}

impl<Message: 'static, const SIZE: usize> EmbassyMailbox<Message, SIZE> {
    pub const fn new() -> Self {
        let channel = Channel::new();
        let lock = AtomicUsize::new(0);

        Self { channel, lock }
    }

    pub fn actor_ref(&'static self) -> ActorRef<Message> {
        ActorRef::new(self)
    }

    pub async fn run<A>(&'static self, mut actor: A) where A: Actor<Message = Message> {
        loop {
            let envelope = self.channel.receive().await;
            let owner = self.lock.load(Ordering::SeqCst);

            if owner == 0 || envelope.caller == Some(owner) {
                actor.handle(envelope.message).await;
            }
        }
    }
}

impl<Message: 'static, const SIZE: usize> Mailbox<Message> for EmbassyMailbox<Message, SIZE> {
    fn address(&self) -> usize {
        from_ref(self).addr()
    }

    fn try_send(&self, envelope: Envelope<Message>) {
        let _ = self.channel.try_send(envelope);
    }

    fn lock(&self, owner: usize) -> bool {
        match self.lock.compare_exchange(0, owner, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => true,
            Err(current_owner) => current_owner == owner,
        }
    }

    fn unlock(&self, owner: usize) -> bool {
        self.lock.compare_exchange(owner, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
}
