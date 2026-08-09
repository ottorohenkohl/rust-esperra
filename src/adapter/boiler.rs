use esperra_domain::{
    actor::Actor,
    boiler::{BoilerActor, BoilerCommand, BoilerEffect},
    output::OutputCommand,
    sensor::TemperatureSensorCommand,
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Receiver, Sender},
};

pub async fn run_boiler_actor<
    const BOILER_N: usize,
    const SENSOR_N: usize,
    const OUTPUT_N: usize,
>(
    actor: &mut BoilerActor,
    messages: Receiver<'_, CriticalSectionRawMutex, BoilerCommand, BOILER_N>,
    temperature_sensor: Sender<'_, CriticalSectionRawMutex, TemperatureSensorCommand, SENSOR_N>,
    heating_element: Sender<'_, CriticalSectionRawMutex, OutputCommand, OUTPUT_N>,
) -> ! {
    loop {
        match actor.handle(messages.receive().await) {
            BoilerEffect::MeasureTemperature(command) => temperature_sensor.send(command).await,
            BoilerEffect::ControlHeatingElement(command) => heating_element.send(command).await,
        }
    }
}
