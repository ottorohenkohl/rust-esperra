use alloc::sync::Arc;
use embassy_sync::channel::DynamicSender;

pub struct ActorRef<Message: 'static> {
    sender: DynamicSender<'static, Message>,
}

impl<Message: 'static> Clone for ActorRef<Message> {
    fn clone(&self) -> Self {
        let sender = self.sender.clone();

        Self { sender }
    }
}

impl<Message: 'static> ActorRef<Message> {
    pub fn new(sender: DynamicSender<'static, Message>) -> Self {
        ActorRef { sender }
    }

    pub fn tell(&self, message: Message) {
        let _ = self.sender.try_send(message);
    }

    pub fn subscriber<PublishedMessage>(&self) -> Subscriber<PublishedMessage> where Message: From<PublishedMessage>, PublishedMessage: 'static {
        let actor_ref = self.clone();

        let tell = Arc::new(move |message| actor_ref.tell(Message::from(message)));

        Subscriber { tell }
    }
}

pub struct Subscriber<Message> {
    tell: Arc<dyn Fn(Message)>,
}

impl<Message> Clone for Subscriber<Message> {
    fn clone(&self) -> Self {
        let tell = Arc::clone(&self.tell);

        Self { tell }
    }
}

impl<Message> Subscriber<Message> {
    pub fn tell(&self, message: Message) {
        (self.tell)(message);
    }
}
