use crate::domain::boiler::{BoilerActor, BoilerMessage};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};

pub async fn run_boiler_actor<S, H>(
    actor: &mut BoilerActor<S, H>,
    messages: Receiver<'_, CriticalSectionRawMutex, BoilerMessage, 4>,
) -> !
where
    S: crate::domain::actor::ActorRef<crate::domain::sensor::TemperatureSensorMessage>,
    H: crate::domain::actor::ActorRef<crate::domain::output::OutputMessage>,
{
    loop {
        actor.handle(messages.receive().await);
    }
}
