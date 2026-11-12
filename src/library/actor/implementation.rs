use super::interface::{Actor, Mailbox};
use super::model::{ActorRef, Envelope};

use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use core::ptr::from_ref;

const UNLOCKED: usize = 0;

pub struct EmbassyMailboxImpl<Message, const SIZE: usize> {
    channel: Channel<CriticalSectionRawMutex, Envelope<Message>, SIZE>,
    lock: AtomicUsize,
}

impl<Message: 'static, const SIZE: usize> EmbassyMailboxImpl<Message, SIZE> {
    pub const fn new() -> Self {
        Self { channel: Channel::new(), lock: AtomicUsize::new(UNLOCKED) }
    }

    pub fn actor_ref(&'static self) -> ActorRef<Message> {
        ActorRef::new(self)
    }

    pub async fn run<A>(&'static self, mut actor: A) where A: Actor<Message = Message> {
        loop {
            let envelope = self.channel.receive().await;
            let owner = self.lock.load(Ordering::SeqCst);

            if envelope.is_system || owner == UNLOCKED || envelope.caller == Some(owner) {
                actor.handle(envelope.message).await;
            }
        }
    }
}

impl<Message: 'static, const SIZE: usize> Mailbox<Message> for EmbassyMailboxImpl<Message, SIZE> {
    fn address(&self) -> usize {
        from_ref(self).addr()
    }

    fn try_send(&self, envelope: Envelope<Message>) {
        let _ = self.channel.try_send(envelope);
    }

    fn lock(&self, owner: usize) -> bool {
        match self.lock.compare_exchange(UNLOCKED, owner, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => true,
            Err(current_owner) => current_owner == owner,
        }
    }

    fn unlock(&self, owner: usize) -> bool {
        self.lock.compare_exchange(owner, UNLOCKED, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
}
