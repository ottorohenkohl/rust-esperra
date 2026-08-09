pub enum MailboxError {
    Full,
}

pub trait ActorRef<Message> {
    fn try_send(&self, message: Message) -> Result<(), MailboxError>;
}
