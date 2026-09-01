use super::interface::{Caller, Mailbox};

use alloc::sync::Arc;

pub struct Envelope<Message> {
    pub caller: Option<usize>,
    pub message: Message,
}

pub struct ActorRef<Message: 'static> {
    mailbox: &'static dyn Mailbox<Message>,
}

impl<Message: 'static> Clone for ActorRef<Message> {
    fn clone(&self) -> Self {
        Self { mailbox: self.mailbox }
    }
}

impl<Message: 'static> ActorRef<Message> {
    pub fn new(mailbox: &'static dyn Mailbox<Message>) -> Self {
        Self { mailbox }
    }

    pub fn tell(&self, caller: Option<&dyn Caller>, message: Message) {
        self.mailbox.try_send(Envelope { caller: caller.map(Caller::address), message });
    }

    pub fn lock(&self, caller: &dyn Caller) -> bool {
        self.mailbox.lock(caller.address())
    }

    pub fn unlock(&self, caller: &dyn Caller) -> bool {
        self.mailbox.unlock(caller.address())
    }

    pub fn subscriber<PublishedMessage, CallerMessage, const SIZE: usize>(&self, caller: &ActorRef<CallerMessage>) -> Subscriber<PublishedMessage, SIZE> where Message: From<PublishedMessage>, PublishedMessage: 'static, CallerMessage: 'static {
        let actor_ref = self.clone();

        let caller = caller.clone();

        let tell = Arc::new(move |message| actor_ref.tell(Some(&caller), Message::from(message)));

        Subscriber { tell }
    }
}

impl<Message: 'static> Caller for ActorRef<Message> {
    fn address(&self) -> usize {
        self.mailbox.address()
    }
}

pub struct Subscriber<Message, const SIZE: usize> {
    tell: Arc<dyn Fn(Message)>,
}

impl<Message, const SIZE: usize> Clone for Subscriber<Message, SIZE> {
    fn clone(&self) -> Self {
        Self { tell: Arc::clone(&self.tell) }
    }
}

impl<Message, const SIZE: usize> Subscriber<Message, SIZE> {
    pub fn tell(&self, message: Message) {
        self.tell.as_ref()(message);
    }
}
