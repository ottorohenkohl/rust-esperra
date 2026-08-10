use alloc::sync::Arc;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::DynamicSender;

pub trait Actor {
    type Message;

    async fn handle(&mut self, message: Self::Message);
}

pub struct ActorRef<Message: 'static> {
    sender: DynamicSender<'static, Message>,
}

impl<Message: 'static> Clone for ActorRef<Message> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}

impl<Message: 'static> ActorRef<Message> {
    pub fn tell(&self, message: Message) {
        let _ = self.sender.try_send(message);
    }

    pub fn subscriber<PublishedMessage>(&self) -> Subscriber<PublishedMessage>
    where
        Message: From<PublishedMessage>,
        PublishedMessage: 'static,
    {
        let actor_ref = self.clone();
        Subscriber { tell: Arc::new(move |message| actor_ref.tell(Message::from(message))) }
    }
}

pub struct Subscriber<Message> {
    tell: Arc<dyn Fn(Message)>,
}

impl<Message> Clone for Subscriber<Message> {
    fn clone(&self) -> Self {
        Self { tell: Arc::clone(&self.tell) }
    }
}

impl<Message> Subscriber<Message> {
    pub fn tell(&self, message: Message) {
        (self.tell)(message);
    }
}

pub struct Mailbox<Message, const CAPACITY: usize> {
    channel: Channel<CriticalSectionRawMutex, Message, CAPACITY>,
}

impl<Message: 'static, const CAPACITY: usize> Mailbox<Message, CAPACITY> {
    pub const fn new() -> Self {
        Self { channel: Channel::new() }
    }

    pub fn actor_ref(&'static self) -> ActorRef<Message> {
        ActorRef { sender: self.channel.dyn_sender() }
    }

    pub async fn run<A>(&'static self, mut actor: A) -> !
    where
        A: Actor<Message = Message>,
    {
        loop {
            actor.handle(self.channel.receive().await).await;
        }
    }
}
