use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum EnergyMessage {
    SetActive(bool),
    CheckInactivity,
    LogActiveState,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum EnergyResponse {
    Active(bool),
}
