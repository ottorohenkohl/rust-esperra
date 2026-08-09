use crate::{
    domain::boiler::{BoilerActor, BoilerMessage},
    library::actor::Actor,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};

pub async fn run_boiler_actor<S, H, I, const N: usize>(
    actor: &mut BoilerActor<S, H, I>,
    messages: Receiver<'_, CriticalSectionRawMutex, BoilerMessage, N>,
) -> !
where
    BoilerActor<S, H, I>: Actor<Message = BoilerMessage>,
{
    loop {
        actor.handle(messages.receive().await);
    }
}
