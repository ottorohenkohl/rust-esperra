use crate::domain::actor::ActorRef;

pub enum OutputMessage {
    SetActive(bool),
    BlinkOnce,
}

pub trait OutputActorRef: ActorRef<OutputMessage> {}

impl<T: ActorRef<OutputMessage>> OutputActorRef for T {}
