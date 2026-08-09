use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use esperra_domain::{
    actor::Actor,
    boiler::{BoilerActor, BoilerCommand},
};

pub async fn run_boiler_actor<S, H, const N: usize>(
    actor: &mut BoilerActor<S, H>,
    messages: Receiver<'_, CriticalSectionRawMutex, BoilerCommand, N>,
) -> !
where
    BoilerActor<S, H>: Actor<Message = BoilerCommand>,
{
    loop {
        actor.handle(messages.receive().await).await;
    }
}
