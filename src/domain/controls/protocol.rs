use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonMessage {
    HandleInputStates,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaterMessage {
    HandleInputState,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum WaterResponse {
    EnoughWater(bool),
}
