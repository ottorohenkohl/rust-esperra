pub trait ActorRef<Message> {
    fn send(&self, message: Message);
}

pub trait Actor {
    type Message;

    fn handle(&mut self, message: Self::Message);
}
