use super::port::{ConnectionState, WeightPort};
use super::protocol::WeightMessage;

use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::ReadConfigPort;
use crate::domain::energy::protocol::EnergyMessage;
use crate::domain::logging::port::LoggingPort;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;

const SHOT_STOP_OFFSET_GRAMS: f64 = 2.0;

pub struct WeightActor<P, L, C> {
    public_name: &'static str,

    weight_port: P,
    logging_port: Arc<L>,
    energy_actor: ActorRef<EnergyMessage>,
    brew_actor: ActorRef<BrewMessage>,
    config_port: Arc<C>,
    shot_weight_armed: bool,
}

impl<P, L, C> WeightActor<P, L, C> {
    pub fn new(public_name: &'static str, weight_port: P, logging_port: Arc<L>, energy_actor: ActorRef<EnergyMessage>, brew_actor: ActorRef<BrewMessage>, config_port: Arc<C>) -> Self {
        Self { public_name, weight_port, logging_port, energy_actor, brew_actor, config_port, shot_weight_armed: false }
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

    async fn watch_for_full_shot(&mut self, weight: f64) {
        let double_shot_weight = self.config_port.read_weight_options().double_shot_weight;

        if self.shot_weight_armed {
            let stop_weight = double_shot_weight - SHOT_STOP_OFFSET_GRAMS;

            if weight >= stop_weight {
                self.brew_actor.tell(None, BrewMessage::SetBrewing(false));
                self.weight_port.reset_ready_state().await;
            }

            return
        }

        self.shot_weight_armed = weight < double_shot_weight;
    }

    async fn shutdown(&mut self) {
        self.weight_port.shutdown().await;
    }

    async fn monitor_weight(&mut self) {
        if self.is_connection_failed().await {
            return
        }

        let weight = self.weight_port.read_weight().await;

        self.logging_port.log(self, None, &weight);

        if self.weight_port.is_ready_state().await {
            self.brew_actor.tell(None, BrewMessage::SetBrewing(true));

            self.watch_for_full_shot(weight).await;
        } else {
            self.shot_weight_armed = false;
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
            WeightMessage::MonitorWeight => self.monitor_weight().await,
            WeightMessage::Shutdown => self.shutdown().await,
        }
    }
}
