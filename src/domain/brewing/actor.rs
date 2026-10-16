
use crate::domain::boiler::protocol::BoilerMessage;
use crate::domain::brewing::protocol::BrewMessage;
use crate::domain::config::port::ReadConfigPort;
use crate::domain::gpio::port::OutputPort;
use crate::library::actor::interface::Actor;
use crate::library::actor::model::ActorRef;

use alloc::sync::Arc;
use embassy_time::{Duration, Instant};

pub struct BrewActor<C, P, V> {
    public_name: &'static str,

    boiler_actor: ActorRef<BoilerMessage>,
    config_port: Arc<C>,
    pump_output_port: P,
    valve_output_port: V,
    brewing: bool,
    pump_active: bool,
    next_pump_change: Option<Instant>,
}

impl<C, P, V> BrewActor<C, P, V> where C: ReadConfigPort, P: OutputPort, V: OutputPort {
    pub fn new(public_name: &'static str, boiler_actor: ActorRef<BoilerMessage>, config_port: Arc<C>, pump_output_port: P, valve_output_port: V) -> Self {
        let brewing = false;
        let pump_active = false;

        let next_pump_change = None;

        Self { public_name, boiler_actor, config_port, pump_output_port, valve_output_port, brewing, pump_active, next_pump_change }
    }

    async fn set_brewing(&mut self, active: bool) {
        if active {
            if self.brewing {
                return;
            }

            let options = self.config_port.read_preinfusion_options();

            self.valve_output_port.set_active(true).await;
            self.pump_output_port.set_active(true).await;

            self.brewing = true;
            self.pump_active = true;

            self.next_pump_change = options.enabled.then(|| Instant::now() + Duration::from_millis(u64::from(options.pump_ms)));
        } else {
            self.brewing = false;
            self.pump_active = false;

            self.next_pump_change = None;

            self.pump_output_port.set_active(false).await;
            self.valve_output_port.set_active(false).await;
        }

        self.boiler_actor.system_tell(BoilerMessage::SetBrewing(active));
    }

    async fn tick(&mut self) {
        if self.next_pump_change.is_some_and(|deadline| Instant::now() >= deadline) {
            self.pump_active ^= true;

            self.pump_output_port.set_active(self.pump_active).await;
            self.boiler_actor.system_tell(BoilerMessage::SetBrewing(self.pump_active));

            self.next_pump_change = match self.pump_active {
                true => None,
                false => Some(Instant::now() + Duration::from_millis(u64::from(self.config_port.read_preinfusion_options().pause_ms)))
            };
        }
    }
}

impl<C, P, V> Actor for BrewActor<C, P, V> where C: ReadConfigPort, P: OutputPort, V: OutputPort {
    type Message = BrewMessage;

    fn name(&self) -> &'static str {
        self.public_name
    }

    async fn handle(&mut self, message: Self::Message) {
        match message {
            BrewMessage::SetBrewing(active) => self.set_brewing(active).await,
            BrewMessage::Tick => self.tick().await,
        }
    }
}
