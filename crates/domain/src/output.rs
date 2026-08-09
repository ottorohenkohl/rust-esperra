#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputState {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMessage {
    SetState(OutputState),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusLightMessage {
    SetState(OutputState),
    BlinkOnce,
}
