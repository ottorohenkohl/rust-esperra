use crate::{
    domain::{
        config::port::{HeaterOptions, PidGains},
        gpio::{output::OutputMessage, sensor::TemperatureSensorMessage},
    },
    library::actor::{Actor, ActorRef},
};

use super::pid::PidService;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoilerMessage {
    EvaluateHeating { cycle_elapsed_ms: u64 },
    RegulateHeating { measured_temperature: f64 },
    StopHeating,
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
    S: ActorRef<TemperatureSensorMessage>,
    H: ActorRef<OutputMessage>,
{
    type Message = BoilerMessage;

    fn handle(&mut self, message: Self::Message) {
        match message {
            BoilerMessage::EvaluateHeating { cycle_elapsed_ms } => {
                self.cycle_elapsed_ms = cycle_elapsed_ms;
                self.temperature_sensor
                    .send(TemperatureSensorMessage::MeasureTemperature);
            }
            BoilerMessage::RegulateHeating {
                measured_temperature,
            } => {
                let active = self.should_heat(measured_temperature);

                self.heating_element.send(OutputMessage::SetActive(active));
            }
            BoilerMessage::StopHeating => {
                self.heating_element.send(OutputMessage::SetActive(false));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::{rc::Rc, vec::Vec};
    use core::cell::RefCell;

    use super::*;

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
        fn send(&self, message: Message) {
            self.messages.borrow_mut().push(message);
        }
    }

    fn actor() -> (
        BoilerActor<RecordingActorRef<TemperatureSensorMessage>, RecordingActorRef<OutputMessage>>,
        RecordingActorRef<TemperatureSensorMessage>,
        RecordingActorRef<OutputMessage>,
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
    fn evaluating_heating_requests_a_temperature_measurement() {
        let (mut actor, temperature_sensor, _) = actor();

        actor.handle(BoilerMessage::EvaluateHeating {
            cycle_elapsed_ms: 10,
        });

        assert_eq!(
            *temperature_sensor.messages.borrow(),
            [TemperatureSensorMessage::MeasureTemperature]
        );
    }

    #[test]
    fn stopping_heating_switches_heater_off() {
        let (mut actor, _, heating_element) = actor();

        actor.handle(BoilerMessage::StopHeating);

        assert_eq!(
            *heating_element.messages.borrow(),
            [OutputMessage::SetActive(false)]
        );
    }

    #[test]
    fn regulating_for_low_temperature_switches_heater_on() {
        let (mut actor, _, heating_element) = actor();

        actor.handle(BoilerMessage::RegulateHeating {
            measured_temperature: 50.0,
        });

        assert_eq!(
            *heating_element.messages.borrow(),
            [OutputMessage::SetActive(true)]
        );
    }
}
