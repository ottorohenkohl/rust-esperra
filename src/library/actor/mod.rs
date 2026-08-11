pub mod mailbox;
pub mod reference;

pub trait Actor {
    type Message;

    async fn handle(&mut self, message: Self::Message);
}
