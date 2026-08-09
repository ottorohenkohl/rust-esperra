use core::future::Future;

pub trait ActorRef<Message> {
    fn send(&self, message: Message) -> impl Future<Output = ()>;
}

pub trait Actor {
    type Message;

    fn handle(&mut self, message: Self::Message) -> impl Future<Output = ()>;
}
