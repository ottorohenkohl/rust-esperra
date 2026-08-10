use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::DynamicSender;

pub trait Actor {
    type Message;

    fn handle(&mut self, message: Self::Message);
}

pub trait AsyncActor {
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
            actor.handle(self.channel.receive().await);
        }
    }

    pub async fn run_async<A>(&'static self, mut actor: A) -> !
    where
        A: AsyncActor<Message = Message>,
    {
        loop {
            actor.handle(self.channel.receive().await).await;
        }
    }
}
