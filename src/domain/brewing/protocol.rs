use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum BrewMessage {
    SetBrewing(bool),
    SetCleaningPump(bool),
    Tick,
}
