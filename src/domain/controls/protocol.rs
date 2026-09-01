#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonMessage {
    HandleInputStates,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaterMessage {
    HandleInputState,
}
