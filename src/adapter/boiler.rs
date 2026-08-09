use crate::{
    adapter::{gpio::HeatingElement, sensor::TemperatureSensorCommand},
    domain::boiler::actor::{BoilerActor, BoilerEffect, BoilerEvent},
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};

pub async fn run_boiler_actor<'d>(
    actor: &mut BoilerActor,
    events: Receiver<'_, CriticalSectionRawMutex, BoilerEvent, 4>,
    sensor_commands: Sender<'_, CriticalSectionRawMutex, TemperatureSensorCommand, 1>,
    heating_element: &mut HeatingElement<'d>,
) -> ! {
    loop {
        let effect = actor.handle(events.receive().await);
        match effect {
            BoilerEffect::ReadTemperature => {
                sensor_commands
                    .send(TemperatureSensorCommand::ReadTemperature)
                    .await;
            }
            BoilerEffect::SetHeating(active) => heating_element.set_active(active),
        }
    }
}

pub type BoilerEventChannel = Channel<CriticalSectionRawMutex, BoilerEvent, 4>;
