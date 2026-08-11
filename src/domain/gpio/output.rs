use crate::library::actor::Actor;
use crate::library::error::Error;
use embassy_time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMessage {
    SetActive(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightMessage {
    SetActive(bool),
    BlinkOnce,
}

pub trait LightPort {
    async fn set_active(&mut self, active: bool) -> Result<(), Error>;
    async fn blink(&mut self, duration: Duration) -> Result<(), Error>;
}

pub struct LightActor<P> {
    port: P,
    blink_duration: Duration,
}

impl<P> LightActor<P> {
    pub fn new(port: P, blink_duration: Duration) -> Self {
        Self { port, blink_duration }
    }
}

impl<P> Actor for LightActor<P>
where
    P: LightPort,
{
    type Message = LightMessage;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            LightMessage::SetActive(active) => {
                let _ = self.port.set_active(active).await;
            }
            LightMessage::BlinkOnce => {
                let _ = self.port.blink(self.blink_duration).await;
            }
        }
    }
}
