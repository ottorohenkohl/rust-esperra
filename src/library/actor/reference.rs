use super::mailbox::Envelope;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_sync::channel::DynamicSender;

mod sealed {
    pub trait Sealed {
        fn address(&self) -> usize;
    }
}

#[doc(hidden)]
pub trait Caller: sealed::Sealed {}

pub struct ActorRef<Message: 'static> {
    sender: DynamicSender<'static, Envelope<Message>>,
    address: usize,
    lock: &'static AtomicUsize,
}

impl<Message: 'static> Clone for ActorRef<Message> {
    fn clone(&self) -> Self {
        let sender = self.sender.clone();

        Self { sender, address: self.address, lock: self.lock }
    }
}

impl<Message: 'static> ActorRef<Message> {
    pub(super) fn new(sender: DynamicSender<'static, Envelope<Message>>, address: usize, lock: &'static AtomicUsize) -> Self {
        ActorRef { sender, address, lock }
    }

    pub fn tell(&self, caller: Option<&dyn Caller>, message: Message) {
        let caller = caller.map(sealed::Sealed::address);
        let _ = self.sender.try_send(Envelope { caller, message });
    }

    pub fn lock(&self, caller: &dyn Caller) -> bool {
        let caller = caller.address();

        match self.lock.compare_exchange(0, caller, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => true,
            Err(owner) => owner == caller,
        }
    }

    pub fn unlock(&self, caller: &dyn Caller) -> bool {
        let caller = caller.address();

        self.lock.compare_exchange(caller, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }

    pub fn subscriber<PublishedMessage, CallerMessage, const SIZE: usize>(&self, caller: &ActorRef<CallerMessage>) -> Subscriber<PublishedMessage, SIZE>
    where
        Message: From<PublishedMessage>,
        PublishedMessage: 'static,
        CallerMessage: 'static,
    {
        let actor_ref = self.clone();
        let caller = caller.clone();

        let tell = Arc::new(move |message| actor_ref.tell(Some(&caller), Message::from(message)));

        Subscriber { tell }
    }
}

impl<Message: 'static> sealed::Sealed for ActorRef<Message> {
    fn address(&self) -> usize {
        self.address
    }
}

impl<Message: 'static> Caller for ActorRef<Message> {}

pub struct Subscriber<Message, const SIZE: usize> {
    tell: Arc<dyn Fn(Message)>,
}

impl<Message, const SIZE: usize> Clone for Subscriber<Message, SIZE> {
    fn clone(&self) -> Self {
        let tell = Arc::clone(&self.tell);

        Self { tell }
    }
}

impl<Message, const SIZE: usize> Subscriber<Message, SIZE> {
    pub fn tell(&self, message: Message) {
        (self.tell)(message);
    }
}
