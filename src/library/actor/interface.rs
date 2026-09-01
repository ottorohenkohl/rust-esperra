use super::implementation::Envelope;

pub trait Actor {
    type Message;

    async fn handle(&mut self, message: Self::Message);
}


pub trait Mailbox<Message: 'static> {
    fn address(&self) -> usize;
    fn try_send(&self, envelope: Envelope<Message>);
    fn lock(&self, owner: usize) -> bool;
    fn unlock(&self, owner: usize) -> bool;
}

pub trait Caller {
    fn address(&self) -> usize;
}
