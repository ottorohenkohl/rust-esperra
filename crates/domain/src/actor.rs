pub trait Actor {
    type Message;
    type Effect;

    fn handle(&mut self, message: Self::Message) -> Self::Effect;
}
