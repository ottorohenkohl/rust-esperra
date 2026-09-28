use super::port::{ConnectionState, WeightPort};
use super::protocol::WeightMessage;

use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::ReadConfigPort;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;

pub struct WeightActor<P, L, C> {
    public_name: &'static str,
    weight_port: P,

    logging_port: Arc<L>,
    energy_actor: ActorRef<EnergyMessage>,
    brew_actor: ActorRef<BrewMessage>,
    config_port: Arc<C>,
}

impl<P, L, C> WeightActor<P, L, C> {
    pub fn new(public_name: &'static str, weight_port: P, logging_port: Arc<L>, energy_actor: ActorRef<EnergyMessage>, brew_actor: ActorRef<BrewMessage>, config_port: Arc<C>) -> Self {
        Self { public_name, weight_port, logging_port, energy_actor, brew_actor, config_port }
    }
}

impl<P, L, C> WeightActor<P, L, C> where P: WeightPort, L: LoggingPort, C: ReadConfigPort {
    async fn is_connection_failed(&mut self) -> bool {
        match self.weight_port.connect().await {
            ConnectionState::AlreadyConnected => false,
            ConnectionState::ConnectionEstablished => {
                self.energy_actor.tell(None, EnergyMessage::SetActive(true));

                false
            },
            ConnectionState::ConnectionFailed => true,
        }
    }

    async fn reset_scale(&mut self) {
        if self.is_connection_failed().await {
            return
        }

        self.weight_port.tare().await;
    }

    async fn watch_for_full_shot(&mut self, weight: f64) {
        if weight >= self.config_port.read_weight_options().double_shot_weight {
            self.brew_actor.tell(None, BrewMessage::SetBrewing(false));

            self.reset_scale().await;
        }
    }

    async fn monitor_weight(&mut self) {
        if self.is_connection_failed().await {
            return
        }

        let weight = self.weight_port.read_weight().await;

        self.logging_port.log(self, None, &weight);

        self.watch_for_full_shot(weight).await;
        self.watch_for_empty_cup(weight).await;
    }

    async fn watch_for_empty_cup(&mut self, weight: f64) {
        let options = self.config_port.read_weight_options();

        if options.automatic_weight_brew_start {
            if 2.0 >= (weight - options.espresso_cup_weight).abs() || (weight - options.capuccino_cup_weight).abs() <= 2.0 {
                self.reset_scale().await;

                self.brew_actor.tell(None, BrewMessage::SetBrewing(true));
            }
        }
    }
}

impl<P, L, C> Actor for WeightActor<P, L, C> where P: WeightPort, L: LoggingPort, C: ReadConfigPort {
    type Message = WeightMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            WeightMessage::ResetScale => self.reset_scale().await,
            WeightMessage::MonitorWeight => self.monitor_weight().await,
        }
    }
}
