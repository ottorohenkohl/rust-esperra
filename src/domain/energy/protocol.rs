use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum EnergyMessage {
    SetActive(bool),
    CheckInactivity,
}
