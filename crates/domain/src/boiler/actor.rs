use crate::{
    actor::{Actor, ActorRef},
    config::port::{HeaterOptions, PidGains},
    output::{OutputCommand, OutputState},
    sensor::{TemperatureSensorCommand, TemperatureSensorEvent},
};

use super::pid::PidService;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerCommand {
    Tick { cycle_elapsed_ms: u64 },
    Temperature(TemperatureSensorEvent),
}

pub struct BoilerActor<S, H> {
    heater_options: HeaterOptions,
    pid_service: PidService,
    temperature_sensor: S,
    heating_element: H,
    cycle_elapsed_ms: u64,
}

impl<S, H> BoilerActor<S, H> {
    pub fn new(
        heater_options: HeaterOptions,
        pid_gains: PidGains,
        temperature_sensor: S,
        heating_element: H,
    ) -> Self {
        Self {
            heater_options,
            pid_service: PidService::new(pid_gains, heater_options.heating_cycle_ms),
            temperature_sensor,
            heating_element,
            cycle_elapsed_ms: 0,
        }
    }

    fn should_heat(&mut self, temperature: f64) -> bool {
        if temperature
            < self.heater_options.target_temperature - self.heater_options.boost_intervall_size
        {
            return true;
        }

        let pid_percentage = self
            .pid_service
            .compute(self.heater_options.target_temperature, temperature);
        let heating_window_ms =
            self.heater_options.heating_cycle_ms as f64 * pid_percentage / 100.0;

        (self.cycle_elapsed_ms as f64) < heating_window_ms
    }
}

impl<S, H> Actor for BoilerActor<S, H>
where
    S: ActorRef<TemperatureSensorCommand>,
    H: ActorRef<OutputCommand>,
{
    type Message = BoilerCommand;

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerCommand::Tick { cycle_elapsed_ms } => {
                self.cycle_elapsed_ms = cycle_elapsed_ms;
                self.temperature_sensor
                    .send(TemperatureSensorCommand::Measure)
                    .await;
            }
            BoilerCommand::Temperature(TemperatureSensorEvent::Measured(temperature)) => {
                let state = if self.should_heat(temperature) {
                    OutputState::Active
                } else {
                    OutputState::Inactive
                };

                self.heating_element.send(OutputCommand::Set(state)).await;
            }
            BoilerCommand::Temperature(TemperatureSensorEvent::MeasurementFailed) => {
                self.heating_element
                    .send(OutputCommand::Set(OutputState::Inactive))
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate std;

    use alloc::{rc::Rc, sync::Arc, vec::Vec};
    use core::{
        cell::RefCell,
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    use super::*;

    struct NoopWake;

    impl std::task::Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    fn block_on(future: impl Future<Output = ()>) {
        let waker = Waker::from(Arc::new(NoopWake));
        let mut context = Context::from_waker(&waker);
        let mut future = pin!(future);

        while future.as_mut().poll(&mut context) == Poll::Pending {
            std::thread::yield_now();
        }
    }

    #[derive(Clone)]
    struct RecordingActorRef<Message> {
        messages: Rc<RefCell<Vec<Message>>>,
    }

    impl<Message> RecordingActorRef<Message> {
        fn new() -> Self {
            Self {
                messages: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl<Message> ActorRef<Message> for RecordingActorRef<Message> {
        async fn send(&self, message: Message) {
            self.messages.borrow_mut().push(message);
        }
    }

    fn actor() -> (
        BoilerActor<RecordingActorRef<TemperatureSensorCommand>, RecordingActorRef<OutputCommand>>,
        RecordingActorRef<TemperatureSensorCommand>,
        RecordingActorRef<OutputCommand>,
    ) {
        let temperature_sensor = RecordingActorRef::new();
        let heating_element = RecordingActorRef::new();
        let actor = BoilerActor::new(
            HeaterOptions {
                heating_cycle_ms: 100_000,
                target_temperature: 65.0,
                boost_intervall_size: 5.0,
            },
            PidGains {
                k_proportional: 1.0,
                k_intergral: 0.0,
                k_derivative: 0.0,
            },
            temperature_sensor.clone(),
            heating_element.clone(),
        );

        (actor, temperature_sensor, heating_element)
    }

    #[test]
    fn tick_requests_a_temperature_measurement() {
        let (mut actor, temperature_sensor, _) = actor();

        block_on(actor.handle(BoilerCommand::Tick {
            cycle_elapsed_ms: 10,
        }));

        assert_eq!(
            *temperature_sensor.messages.borrow(),
            [TemperatureSensorCommand::Measure]
        );
    }

    #[test]
    fn sensor_failure_switches_heater_off() {
        let (mut actor, _, heating_element) = actor();

        block_on(actor.handle(BoilerCommand::Temperature(
            TemperatureSensorEvent::MeasurementFailed,
        )));

        assert_eq!(
            *heating_element.messages.borrow(),
            [OutputCommand::Set(OutputState::Inactive)]
        );
    }

    #[test]
    fn low_temperature_switches_heater_on() {
        let (mut actor, _, heating_element) = actor();

        block_on(actor.handle(BoilerCommand::Temperature(
            TemperatureSensorEvent::Measured(50.0),
        )));

        assert_eq!(
            *heating_element.messages.borrow(),
            [OutputCommand::Set(OutputState::Active)]
        );
    }
}
