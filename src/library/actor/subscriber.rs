use alloc::sync::Arc;

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

